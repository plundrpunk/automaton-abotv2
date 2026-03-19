/**
 * AMS MCP Bridge - SSE Client → Stdio Server
 *
 * Connects to the AMS MCP SSE endpoint as a client and re-exposes
 * all discovered tools through a stdio MCP server that the Claude
 * Agent SDK can consume.
 *
 * Environment:
 *   AMS_MCP_ENDPOINT - SSE endpoint URL (e.g. http://host.docker.internal:3002/sse)
 *   AMS_AGENT_TOKEN  - Bearer token for authentication
 *   AMS_TOOL_MODE    - "minimal" | "full" (optional, defaults to "full")
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { SSEClientTransport } from '@modelcontextprotocol/sdk/client/sse.js';
import {
  ListToolsRequestSchema,
  CallToolRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';

const AMS_MCP_ENDPOINT = process.env.AMS_MCP_ENDPOINT || '';
const AMS_AGENT_TOKEN = process.env.AMS_AGENT_TOKEN || '';
const AMS_TOOL_MODE = process.env.AMS_TOOL_MODE || 'full';

const MINIMAL_TOOLS = new Set([
  'search_memories',
  'create_memory',
  'execute_automaton',
  'list_automata',
  'suggest_automaton',
  'search_tools',
  'gateway_send_message',
  'gateway_call_tool',
  'gateway_list_servers',
  'gateway_list_sessions',
  'agent_bootstrap',
]);

const RECONNECT_DELAY_MS = 3000;
const MAX_RECONNECT_ATTEMPTS = 5;

function log(message: string): void {
  console.error(`[ams-mcp-bridge] ${message}`);
}

interface UpstreamTool {
  name: string;
  description?: string;
  inputSchema: Record<string, unknown>;
}

let upstreamClient: Client | null = null;
let cachedTools: UpstreamTool[] = [];
let connected = false;

async function connectUpstream(): Promise<void> {
  if (!AMS_MCP_ENDPOINT) {
    log('AMS_MCP_ENDPOINT not set, bridge will return empty tool list');
    return;
  }

  for (let attempt = 1; attempt <= MAX_RECONNECT_ATTEMPTS; attempt++) {
    try {
      log(`Connecting to upstream SSE (attempt ${attempt}/${MAX_RECONNECT_ATTEMPTS}): ${AMS_MCP_ENDPOINT}`);

      const url = new URL(AMS_MCP_ENDPOINT);
      const headers: Record<string, string> = {};
      if (AMS_AGENT_TOKEN) {
        headers['Authorization'] = `Bearer ${AMS_AGENT_TOKEN}`;
      }

      const transport = new SSEClientTransport(url, {
        requestInit: { headers },
        eventSourceInit: {
          fetch: (input: string | URL | Request, init?: RequestInit) => {
            const mergedInit = { ...init, headers: { ...headers, ...(init?.headers as Record<string, string>) } };
            return fetch(input, mergedInit);
          },
        },
      });

      const client = new Client(
        { name: 'ams-mcp-bridge', version: '1.0.0' },
        { capabilities: {} },
      );

      await client.connect(transport);
      upstreamClient = client;
      connected = true;

      log('Connected to upstream SSE server');
      await discoverTools();
      return;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      log(`Connection attempt ${attempt} failed: ${msg}`);
      if (attempt < MAX_RECONNECT_ATTEMPTS) {
        await sleep(RECONNECT_DELAY_MS);
      }
    }
  }

  log(`Failed to connect after ${MAX_RECONNECT_ATTEMPTS} attempts. Bridge will operate with empty tool list.`);
}

async function discoverTools(): Promise<void> {
  if (!upstreamClient) return;

  try {
    const result = await upstreamClient.listTools();
    let tools = (result.tools || []) as UpstreamTool[];

    if (AMS_TOOL_MODE === 'minimal') {
      tools = tools.filter((t) => MINIMAL_TOOLS.has(t.name));
      log(`Filtered to ${tools.length} minimal-mode tools`);
    }

    cachedTools = tools;
    log(`Discovered ${cachedTools.length} tools from upstream`);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    log(`Failed to discover tools: ${msg}`);
    cachedTools = [];
  }
}

async function ensureConnected(): Promise<void> {
  if (connected && upstreamClient) return;
  await connectUpstream();
}

async function callUpstreamTool(
  name: string,
  args: Record<string, unknown>,
): Promise<{ content: Array<{ type: string; text: string }>; isError?: boolean }> {
  if (!upstreamClient || !connected) {
    await connectUpstream();
    if (!upstreamClient || !connected) {
      return {
        content: [{ type: 'text', text: 'AMS MCP bridge is not connected to upstream server.' }],
        isError: true,
      };
    }
  }

  try {
    const result = await upstreamClient.callTool({ name, arguments: args });
    return result as { content: Array<{ type: string; text: string }>; isError?: boolean };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    log(`Tool call '${name}' failed: ${msg}`);

    if (msg.includes('ECONNREFUSED') || msg.includes('ECONNRESET') || msg.includes('fetch failed')) {
      connected = false;
      upstreamClient = null;
    }

    return {
      content: [{ type: 'text', text: `AMS MCP bridge error calling '${name}': ${msg}` }],
      isError: true,
    };
  }
}

const server = new Server(
  { name: 'ams', version: '1.0.0' },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
  await ensureConnected();
  return {
    tools: cachedTools.map((t) => ({
      name: t.name,
      description: t.description,
      inputSchema: t.inputSchema,
    })),
  };
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name } = request.params;
  const args = (request.params.arguments || {}) as Record<string, unknown>;

  log(`Forwarding tool call: ${name}`);
  return await callUpstreamTool(name, args);
});

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main(): Promise<void> {
  log('Starting AMS MCP Bridge...');
  log(`Endpoint: ${AMS_MCP_ENDPOINT || '(not set)'}`);
  log(`Tool mode: ${AMS_TOOL_MODE}`);
  log(`Auth token: ${AMS_AGENT_TOKEN ? 'present' : 'not set'}`);

  connectUpstream().catch((err) => {
    log(`Background connect error: ${err instanceof Error ? err.message : String(err)}`);
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((err) => {
  log(`Fatal error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
