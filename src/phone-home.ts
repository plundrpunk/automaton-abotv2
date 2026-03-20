/**
 * Phone Home Protocol for AutomatonAbot Fleet
 * Handles heartbeat loop, execution streaming, and message polling from AMS
 */

import os from 'os';
import { AMSClient, AgentMessage, getAMSClient } from './ams-client.js';
import { logger } from './logger.js';

interface UsageAccumulator {
  tokensIn: number;
  tokensOut: number;
  executions: number;
}

interface PhoneHomeState {
  isWorking: boolean;
  currentExecutionId: string | null;
  pendingTasks: number;
  contextUsagePercent: number;
  usage: UsageAccumulator;
}

const state: PhoneHomeState = {
  isWorking: false,
  currentExecutionId: null,
  pendingTasks: 0,
  contextUsagePercent: 0,
  usage: {
    tokensIn: 0,
    tokensOut: 0,
    executions: 0,
  },
};

let heartbeatInterval: ReturnType<typeof setInterval> | null = null;
let messagePollInterval: ReturnType<typeof setInterval> | null = null;

type MessageHandler = (messages: AgentMessage[]) => void;
let messageHandler: MessageHandler | null = null;

export function onDashboardMessages(handler: MessageHandler): void {
  messageHandler = handler;
}

export function startPhoneHome(intervalMs: number = 30000): void {
  const client = getAMSClient();
  if (!client) {
    logger.warn('AMS client not initialized, skipping phone-home');
    return;
  }

  logger.info({ intervalMs }, 'Starting phone-home heartbeat loop');

  sendHeartbeat(client);

  heartbeatInterval = setInterval(() => {
    sendHeartbeat(client);
  }, intervalMs);

  messagePollInterval = setInterval(() => {
    pollMessages(client);
  }, 3000);

  logger.info('Message polling started (3s interval)');
}

export function stopPhoneHome(): void {
  if (heartbeatInterval) {
    clearInterval(heartbeatInterval);
    heartbeatInterval = null;
    logger.info('Stopped phone-home heartbeat loop');
  }
  if (messagePollInterval) {
    clearInterval(messagePollInterval);
    messagePollInterval = null;
    logger.info('Stopped message polling');
  }
}

async function sendHeartbeat(client: AMSClient): Promise<void> {
  const memUsage = process.memoryUsage();
  const cpuUsage = os.loadavg()[0] / os.cpus().length * 100;

  const usage = flushUsage();

  await client.sendHeartbeat({
    containerId: process.env.HOSTNAME || os.hostname(),
    status: state.isWorking ? 'working' : 'idle',
    metrics: {
      memoryUsageMb: Math.round(memUsage.heapUsed / 1024 / 1024),
      cpuPercent: Math.round(cpuUsage),
      uptimeSeconds: client.getUptimeSeconds(),
      pendingTasks: state.pendingTasks,
      contextUsagePercent: state.contextUsagePercent,
    },
    usage,
  });
}

async function pollMessages(client: AMSClient): Promise<void> {
  try {
    const messages = await client.pollMessages();
    if (messages.length > 0 && messageHandler) {
      messageHandler(messages);
    }
  } catch {
    // Silent — poll failures are expected during transient outages
  }
}

function flushUsage(): { tokensInSinceLastHeartbeat: number; tokensOutSinceLastHeartbeat: number; executionsSinceLastHeartbeat: number } {
  const usage = {
    tokensInSinceLastHeartbeat: state.usage.tokensIn,
    tokensOutSinceLastHeartbeat: state.usage.tokensOut,
    executionsSinceLastHeartbeat: state.usage.executions,
  };
  state.usage = { tokensIn: 0, tokensOut: 0, executions: 0 };
  return usage;
}

export async function executionStarted(
  executionId: string,
  prompt: string,
  agentName?: string,
): Promise<void> {
  const client = getAMSClient();
  state.isWorking = true;
  state.currentExecutionId = executionId;

  if (client) {
    await client.registerExecution({
      executionId,
      agentName: agentName || 'AbotPrime',
      task: prompt.slice(0, 200),
      model: 'claude-code-oauth',
    });

    await client.emitExecutionChunk({
      executionId,
      type: 'start',
      data: { content: prompt.slice(0, 500) },
    });
  }
}

export function emitOutput(executionId: string, content: string, model?: string): void {
  const client = getAMSClient();
  if (client) {
    client.emitExecutionChunk({
      executionId,
      type: 'output',
      data: { content, model },
    });
  }
}

export function emitToolCall(executionId: string, toolName: string, toolInput: object): void {
  const client = getAMSClient();
  if (client) {
    client.emitExecutionChunk({
      executionId,
      type: 'tool_call',
      data: { toolName, toolInput },
    });
  }
}

export function emitToolResult(executionId: string, toolOutput: string): void {
  const client = getAMSClient();
  if (client) {
    client.emitExecutionChunk({
      executionId,
      type: 'tool_result',
      data: { toolOutput: toolOutput.slice(0, 2000) },
    });
  }
}

export function executionComplete(
  executionId: string,
  durationMs: number,
  tokensIn?: number,
  tokensOut?: number,
): void {
  const client = getAMSClient();
  state.isWorking = false;
  state.currentExecutionId = null;
  state.usage.executions += 1;

  if (tokensIn) state.usage.tokensIn += tokensIn;
  if (tokensOut) state.usage.tokensOut += tokensOut;

  if (client) {
    client.emitExecutionChunk({
      executionId,
      type: 'complete',
      data: { durationMs, tokensIn, tokensOut },
    });
    client.completeExecution(executionId, { tokensIn, tokensOut });
  }
}

export function executionError(executionId: string, error: string): void {
  const client = getAMSClient();
  state.isWorking = false;
  state.currentExecutionId = null;

  if (client) {
    client.emitExecutionChunk({
      executionId,
      type: 'error',
      data: { error },
    });
  }
}

export function setPendingTasks(count: number): void {
  state.pendingTasks = count;
}

export function setContextUsage(percent: number): void {
  state.contextUsagePercent = percent;
}

export function addTokenUsage(tokensIn: number, tokensOut: number): void {
  state.usage.tokensIn += tokensIn;
  state.usage.tokensOut += tokensOut;
}
