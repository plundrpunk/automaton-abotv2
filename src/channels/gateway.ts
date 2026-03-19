/**
 * AMS Communications Gateway Channel Adapter
 *
 * Connects to the AMS Communications Gateway to receive and send messages
 * across all supported channels (Telegram, Discord, Slack, WhatsApp).
 *
 * The gateway normalizes all channel messages into a common format.
 * This adapter polls for inbound messages and sends responses back
 * through the gateway's broadcast API.
 */

import { readEnvFile } from '../env.js';
import { logger } from '../logger.js';
import { Channel, NewMessage, OnChatMetadata, OnInboundMessage } from '../types.js';
import { RegisteredGroup } from '../types.js';

const envVars = readEnvFile(['AMS_GATEWAY_URL', 'AMS_URL', 'AMS_AGENT_TOKEN', 'GATEWAY_API_KEY', 'GATEWAY_POLL_INTERVAL_MS']);

interface GatewayConfig {
  gatewayUrl: string;
  apiKey: string;
  pollIntervalMs: number;
}

interface GatewayInboundMessage {
  id: string;
  channel: string;
  sender_id: string;
  sender_name: string;
  content: string;
  timestamp: string;
  attachments?: string[];
  reply_to?: string;
}

interface GatewaySession {
  key: string;
  channel: string;
  sender_id: string;
  agent_name: string;
  history_entries: number;
  created_at: string;
  last_seen_at: string;
}

interface ChannelCallbacks {
  onMessage: (chatJid: string, msg: NewMessage) => void;
  onChatMetadata: OnChatMetadata;
  registeredGroups: () => Record<string, RegisteredGroup>;
}

export class GatewayChannel implements Channel {
  name = 'gateway';
  private config: GatewayConfig;
  private callbacks: ChannelCallbacks;
  private connected = false;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private knownJids = new Set<string>();
  private lastPollTime: string = new Date(0).toISOString();

  constructor(callbacks: ChannelCallbacks) {
    this.callbacks = callbacks;
    this.config = {
      gatewayUrl: process.env.AMS_GATEWAY_URL || envVars.AMS_GATEWAY_URL || (process.env.AMS_URL || envVars.AMS_URL || '').replace(/:3001$/, ':18800') || 'http://localhost:18800',
      apiKey: process.env.AMS_AGENT_TOKEN || envVars.AMS_AGENT_TOKEN || process.env.GATEWAY_API_KEY || envVars.GATEWAY_API_KEY || '',
      pollIntervalMs: parseInt(process.env.GATEWAY_POLL_INTERVAL_MS || envVars.GATEWAY_POLL_INTERVAL_MS || '2000', 10),
    };
  }

  async connect(): Promise<void> {
    logger.info({ gatewayUrl: this.config.gatewayUrl }, 'Connecting to AMS Communications Gateway');

    // Verify gateway is reachable
    try {
      const response = await this.fetchGateway('/gateway/health');
      if (response.ok) {
        const health = await response.json() as { status: string; enabled_channels?: string[] };
        logger.info(
          { status: health.status, channels: health.enabled_channels },
          'Gateway connected',
        );
        this.connected = true;
      } else {
        logger.warn({ status: response.status }, 'Gateway health check failed, will retry');
        this.connected = true; // Still start polling — gateway may come up later
      }
    } catch (err) {
      logger.warn({ err }, 'Gateway not reachable, will retry on poll');
      this.connected = true; // Start polling anyway
    }

    // Start polling for inbound messages
    this.startPolling();

    // Discover existing sessions and register their JIDs
    await this.discoverSessions();
  }

  async sendMessage(jid: string, text: string): Promise<void> {
    // Parse channel and recipient from JID format: "channel:recipient_id"
    const [channel, recipient] = this.parseJid(jid);
    if (!channel || !recipient) {
      logger.warn({ jid }, 'Cannot send message: invalid JID format');
      return;
    }

    try {
      const response = await this.fetchGateway('/gateway/broadcast', {
        method: 'POST',
        body: JSON.stringify({
          channel,
          message: text,
          recipients: [recipient],
        }),
      });

      if (!response.ok) {
        const body = await response.text().catch(() => '');
        logger.warn({ jid, status: response.status, body }, 'Gateway send failed');
      }
    } catch (err) {
      logger.error({ err, jid }, 'Failed to send message via gateway');
    }
  }

  isConnected(): boolean {
    return this.connected;
  }

  ownsJid(jid: string): boolean {
    // Gateway owns all JIDs with channel prefix format (e.g., "telegram:123", "discord:456")
    return jid.includes(':') || this.knownJids.has(jid);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    logger.info('Gateway channel disconnected');
  }

  async setTyping?(jid: string, isTyping: boolean): Promise<void> {
    // Gateway doesn't support typing indicators directly
  }

  // --- Private ---

  private async fetchGateway(path: string, options: RequestInit = {}): Promise<Response> {
    const url = `${this.config.gatewayUrl}${path}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(this.config.apiKey ? { 'X-API-Key': this.config.apiKey, 'Authorization': `Bearer ${this.config.apiKey}` } : {}),
      ...(options.headers as Record<string, string> || {}),
    };

    return fetch(url, { ...options, headers });
  }

  private startPolling(): void {
    if (this.pollTimer) return;

    this.pollTimer = setInterval(() => {
      this.pollInbound().catch(err => {
        logger.debug({ err }, 'Gateway poll error');
      });
    }, this.config.pollIntervalMs);

    logger.info({ intervalMs: this.config.pollIntervalMs }, 'Gateway polling started');
  }

  private async pollInbound(): Promise<void> {
    try {
      // Poll the gateway's inbound endpoint for messages directed to this abot
      const response = await this.fetchGateway(`/gateway/inbound/poll?since=${encodeURIComponent(this.lastPollTime)}`);

      if (!response.ok) {
        // If the poll endpoint doesn't exist, that's OK — messages come via fleet polling
        if (response.status !== 404) {
          logger.debug({ status: response.status }, 'Gateway poll returned non-OK');
        }
        return;
      }

      const data = await response.json() as { messages?: GatewayInboundMessage[] };
      if (!data.messages || data.messages.length === 0) return;

      for (const msg of data.messages) {
        const jid = `${msg.channel}:${msg.sender_id}`;
        this.knownJids.add(jid);

        // Report chat metadata
        this.callbacks.onChatMetadata(
          jid,
          msg.timestamp,
          msg.sender_name,
          msg.channel,
          true,
        );

        // Deliver as NewMessage
        const newMessage: NewMessage = {
          id: msg.id || `${msg.channel}-${msg.sender_id}-${Date.now()}`,
          chat_jid: jid,
          sender: msg.sender_id,
          sender_name: msg.sender_name || msg.sender_id,
          content: msg.content,
          timestamp: msg.timestamp || new Date().toISOString(),
          is_from_me: false,
          is_bot_message: false,
        };

        this.callbacks.onMessage(jid, newMessage);
        this.lastPollTime = msg.timestamp;
      }
    } catch {
      // Silent — connection errors during polling are normal during startup
    }
  }

  private async discoverSessions(): Promise<void> {
    try {
      const response = await this.fetchGateway('/gateway/sessions');
      if (!response.ok) return;

      const data = await response.json() as { sessions?: GatewaySession[] };
      if (!data.sessions) return;

      for (const session of data.sessions) {
        const jid = `${session.channel}:${session.sender_id}`;
        this.knownJids.add(jid);
        this.callbacks.onChatMetadata(
          jid,
          session.last_seen_at,
          undefined,
          session.channel,
          true,
        );
      }

      logger.info({ count: data.sessions.length }, 'Discovered gateway sessions');
    } catch {
      // Gateway may not be up yet — that's fine
    }
  }

  private parseJid(jid: string): [string | null, string | null] {
    const colonIdx = jid.indexOf(':');
    if (colonIdx === -1) return [null, null];
    return [jid.slice(0, colonIdx), jid.slice(colonIdx + 1)];
  }
}
