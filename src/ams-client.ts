/**
 * AMS Client for AutomatonAbot Fleet
 * Handles communication with AMS mothership
 */

import { logger } from './logger.js';

export interface AMSConfig {
  endpoint: string;
  agentId: string;
  tenantId: string;
  agentToken: string;
  instanceId: string;
}

export interface HeartbeatPayload {
  agentId: string;
  tenantId: string;
  containerId: string;
  timestamp: string;
  status: 'idle' | 'working' | 'error';
  metrics: {
    memoryUsageMb: number;
    cpuPercent: number;
    uptimeSeconds: number;
    pendingTasks: number;
    contextUsagePercent: number;
  };
  usage: {
    tokensInSinceLastHeartbeat: number;
    tokensOutSinceLastHeartbeat: number;
    executionsSinceLastHeartbeat: number;
  };
}

export interface ExecutionChunk {
  agentId: string;
  tenantId: string;
  executionId: string;
  type: 'start' | 'output' | 'tool_call' | 'tool_result' | 'complete' | 'error';
  timestamp: string;
  data: {
    content?: string;
    toolName?: string;
    toolInput?: object;
    toolOutput?: string;
    tokensIn?: number;
    tokensOut?: number;
    durationMs?: number;
    error?: string;
    model?: string;
  };
}

export interface MemoryPayload {
  title: string;
  content: string;
  memoryTier: 'episodic' | 'semantic' | 'procedural';
  entityType?: string;
  importance?: number;
  tags?: string[];
  scopeHint?: 'global' | 'agent';
}

export interface ContinuationPayload {
  originalGoal: string;
  nextAction: string;
  completedSubtasks?: string[];
  remainingSubtasks?: string[];
  handoffNotes?: string;
  contextUsageAtDeath?: number;
}

export interface AgentMessage {
  id: string;
  content: string;
  type: 'guidance' | 'intervention' | 'command';
  sender: string;
  timestamp: string;
}

export class AMSClient {
  private config: AMSConfig;
  private startTime: number;
  private executionMap: Map<string, string> = new Map();

  constructor(config: AMSConfig) {
    this.config = config;
    this.startTime = Date.now();
  }

  private async fetch(path: string, options: RequestInit = {}): Promise<Response> {
    const url = `${this.config.endpoint}${path}`;
    const headers: Record<string, string> = {
      'X-API-Key': this.config.agentToken,
      'Authorization': `Bearer ${this.config.agentToken}`,
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string> || {}),
    };

    try {
      const response = await fetch(url, { ...options, headers });
      return response;
    } catch (err) {
      logger.error({ err, path }, 'AMS request failed');
      throw err;
    }
  }

  async healthCheck(): Promise<boolean> {
    try {
      const response = await this.fetch('/api/health');
      return response.ok;
    } catch {
      return false;
    }
  }

  async sendHeartbeat(payload: Omit<HeartbeatPayload, 'agentId' | 'tenantId' | 'timestamp'>): Promise<void> {
    const fullPayload: HeartbeatPayload = {
      ...payload,
      agentId: this.config.agentId,
      tenantId: this.config.tenantId,
      timestamp: new Date().toISOString(),
    };

    try {
      const response = await this.fetch('/api/fleet/heartbeat', {
        method: 'POST',
        body: JSON.stringify(fullPayload),
      });

      if (!response.ok) {
        const text = await response.text().catch(() => '');
        logger.warn({ status: response.status, body: text }, 'Heartbeat rejected');
      }
    } catch (err) {
      logger.error({ err }, 'Failed to send heartbeat');
    }
  }

  async registerAgent(agentName?: string, metadata: Record<string, unknown> = {}): Promise<boolean> {
    try {
      const response = await this.fetch('/api/fleet/agents', {
        method: 'POST',
        body: JSON.stringify({
          agentId: this.config.agentId,
          tenantId: this.config.tenantId,
          agentName: agentName || this.config.agentId,
          instanceId: this.config.instanceId,
          metadata: {
            instanceId: this.config.instanceId,
            ...metadata,
          },
        }),
      });

      if (!response.ok) {
        const text = await response.text().catch(() => '');
        logger.warn({ status: response.status, body: text }, 'Agent registration rejected');
        return false;
      }

      return true;
    } catch (err) {
      logger.error({ err }, 'Failed to register agent');
      return false;
    }
  }

  async registerExecution(opts: {
    executionId: string;
    agentName: string;
    task: string;
    model?: string;
  }): Promise<string | null> {
    try {
      const response = await this.fetch('/api/fleet/executions/register', {
        method: 'POST',
        body: JSON.stringify({
          agentId: this.config.agentId,
          tenantId: this.config.tenantId,
          executionId: opts.executionId,
          agentName: opts.agentName,
          task: opts.task,
          model: opts.model || 'claude-code-oauth',
          instanceId: this.config.instanceId,
        }),
      });

      if (response.ok) {
        const data = await response.json() as { executionId?: string; fleetExecutionId?: string };
        const registryId = data.executionId || opts.executionId;
        this.executionMap.set(opts.executionId, registryId);
        logger.info(
          { fleetId: opts.executionId, registryId },
          'Execution registered with Observatory',
        );
        return registryId;
      }

      const text = await response.text().catch(() => '');
      logger.warn({ status: response.status, body: text }, 'Execution registration failed');
      return null;
    } catch (err) {
      logger.error({ err, executionId: opts.executionId }, 'Failed to register execution');
      return null;
    }
  }

  getRegistryId(fleetExecutionId: string): string {
    return this.executionMap.get(fleetExecutionId) || fleetExecutionId;
  }

  async emitExecutionChunk(chunk: Omit<ExecutionChunk, 'agentId' | 'tenantId' | 'timestamp'>): Promise<void> {
    const registryId = this.getRegistryId(chunk.executionId);

    const fullChunk: ExecutionChunk = {
      ...chunk,
      executionId: registryId,
      agentId: this.config.agentId,
      tenantId: this.config.tenantId,
      timestamp: new Date().toISOString(),
    };

    try {
      const response = await this.fetch(`/api/fleet/executions/${registryId}/emit`, {
        method: 'POST',
        body: JSON.stringify(fullChunk),
      });
      if (!response.ok) {
        const text = await response.text().catch(() => '');
        logger.warn(
          { status: response.status, body: text, executionId: registryId, chunkType: chunk.type },
          'Execution chunk rejected',
        );
      }
    } catch (err) {
      logger.error({ err, executionId: chunk.executionId }, 'Failed to emit execution chunk');
    }
  }

  async completeExecution(executionId: string, stats?: {
    tokensIn?: number;
    tokensOut?: number;
    cost?: number;
  }): Promise<void> {
    const registryId = this.getRegistryId(executionId);
    try {
      const response = await this.fetch(`/api/fleet/executions/${registryId}/complete`, {
        method: 'POST',
        body: JSON.stringify(stats || {}),
      });
      if (!response.ok) {
        const text = await response.text().catch(() => '');
        logger.warn(
          { status: response.status, body: text, executionId: registryId },
          'Execution completion rejected',
        );
      }
    } catch (err) {
      logger.error({ err, executionId }, 'Failed to complete execution');
    }
  }

  async sendDashboardResponse(content: string): Promise<void> {
    try {
      await this.fetch(
        `/api/fleet/agents/${encodeURIComponent(this.config.agentId)}/messages`,
        {
          method: 'POST',
          body: JSON.stringify({
            content,
            type: 'response',
            sender: 'agent',
            recipient: 'dashboard',
          }),
        },
      );
    } catch (err) {
      logger.error({ err }, 'Failed to send dashboard response');
    }
  }

  async pollMessages(): Promise<AgentMessage[]> {
    try {
      const response = await this.fetch(
        `/api/fleet/agents/${this.config.agentId}/messages?recipient=agent&instance_id=${encodeURIComponent(this.config.instanceId)}`,
      );

      if (response.ok) {
        const data = await response.json() as { messages?: AgentMessage[]; count?: number };
        if (data.messages && data.messages.length > 0) {
          logger.info({ count: data.count }, 'Received messages from Dashboard');
        }
        return data.messages || [];
      }
      return [];
    } catch (err) {
      logger.debug({ err }, 'Message poll failed');
      return [];
    }
  }

  async createMemory(payload: MemoryPayload): Promise<string | null> {
    try {
      const response = await this.fetch('/api/fleet/memory/sync', {
        method: 'POST',
        body: JSON.stringify({
          agentId: this.config.agentId,
          tenantId: this.config.tenantId,
          memories: [payload],
        }),
      });

      if (response.ok) {
        const data = await response.json() as { memoryIds?: string[] };
        return data.memoryIds?.[0] || null;
      }
      return null;
    } catch (err) {
      logger.error({ err }, 'Failed to create memory');
      return null;
    }
  }

  async createContinuation(payload: ContinuationPayload): Promise<string | null> {
    try {
      const response = await this.fetch('/api/fleet/continuations', {
        method: 'POST',
        body: JSON.stringify({
          agentId: this.config.agentId,
          tenantId: this.config.tenantId,
          ...payload,
        }),
      });

      if (response.ok) {
        const data = await response.json() as { continuationId?: string };
        return data.continuationId || null;
      }
      return null;
    } catch (err) {
      logger.error({ err }, 'Failed to create continuation');
      return null;
    }
  }

  async claimContinuation(continuationId: string): Promise<ContinuationPayload | null> {
    try {
      const response = await this.fetch(`/api/fleet/continuations/${continuationId}/claim`, {
        method: 'POST',
        body: JSON.stringify({
          agentId: this.config.agentId,
          tenantId: this.config.tenantId,
        }),
      });

      if (response.ok) {
        return await response.json() as ContinuationPayload;
      }
      return null;
    } catch (err) {
      logger.error({ err, continuationId }, 'Failed to claim continuation');
      return null;
    }
  }

  getUptimeSeconds(): number {
    return Math.floor((Date.now() - this.startTime) / 1000);
  }
}

// Singleton instance (initialized when Fleet mode is enabled)
let amsClient: AMSClient | null = null;

export function initAMSClient(config: AMSConfig): AMSClient {
  amsClient = new AMSClient(config);
  return amsClient;
}

export function getAMSClient(): AMSClient | null {
  return amsClient;
}
