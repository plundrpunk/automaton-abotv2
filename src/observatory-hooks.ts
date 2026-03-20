/**
 * Observatory Hooks for AutomatonAbot
 * Provides real-time execution visibility to the AMS dashboard
 */

import { getAMSClient, ExecutionChunk } from './ams-client.js';
import { logger } from './logger.js';

interface ObservatoryConfig {
  streamOutput: boolean;
  streamToolCalls: boolean;
  maxContentLength: number;
}

const defaultConfig: ObservatoryConfig = {
  streamOutput: true,
  streamToolCalls: true,
  maxContentLength: 2000,
};

let config: ObservatoryConfig = { ...defaultConfig };

export function configureObservatory(options: Partial<ObservatoryConfig>): void {
  config = { ...config, ...options };
  logger.debug({ config }, 'Observatory configured');
}

export async function streamOutput(
  executionId: string,
  content: string,
  model?: string,
): Promise<void> {
  if (!config.streamOutput) return;

  const client = getAMSClient();
  if (!client) return;

  const truncatedContent = content.length > config.maxContentLength
    ? content.slice(0, config.maxContentLength) + '...'
    : content;

  await client.emitExecutionChunk({
    executionId,
    type: 'output',
    data: { content: truncatedContent, model },
  });
}

export async function streamToolCall(
  executionId: string,
  toolName: string,
  toolInput: object,
): Promise<void> {
  if (!config.streamToolCalls) return;

  const client = getAMSClient();
  if (!client) return;

  await client.emitExecutionChunk({
    executionId,
    type: 'tool_call',
    data: { toolName, toolInput },
  });
}

export async function streamToolResult(
  executionId: string,
  toolOutput: string,
): Promise<void> {
  if (!config.streamToolCalls) return;

  const client = getAMSClient();
  if (!client) return;

  const truncatedOutput = toolOutput.length > config.maxContentLength
    ? toolOutput.slice(0, config.maxContentLength) + '...'
    : toolOutput;

  await client.emitExecutionChunk({
    executionId,
    type: 'tool_result',
    data: { toolOutput: truncatedOutput },
  });
}

export async function reportMetrics(
  executionId: string,
  metrics: {
    tokensIn?: number;
    tokensOut?: number;
    durationMs?: number;
    contextPercent?: number;
  },
): Promise<void> {
  const client = getAMSClient();
  if (!client) return;

  logger.debug({ executionId, metrics }, 'Reporting metrics to Observatory');

  await client.emitExecutionChunk({
    executionId,
    type: 'output',
    data: {
      content: `[Metrics] tokens: ${metrics.tokensIn || 0}/${metrics.tokensOut || 0}, context: ${metrics.contextPercent || 0}%`,
      tokensIn: metrics.tokensIn,
      tokensOut: metrics.tokensOut,
      durationMs: metrics.durationMs,
    },
  });
}

export async function reportError(
  executionId: string,
  error: Error | string,
  context?: Record<string, unknown>,
): Promise<void> {
  const client = getAMSClient();
  if (!client) return;

  const errorMessage = error instanceof Error ? error.message : String(error);
  const errorStack = error instanceof Error ? error.stack : undefined;

  logger.error({ executionId, error: errorMessage, context }, 'Reporting error to Observatory');

  await client.emitExecutionChunk({
    executionId,
    type: 'error',
    data: {
      error: errorMessage,
      content: errorStack ? `Stack: ${errorStack.slice(0, 500)}` : undefined,
    },
  });
}

export function createExecutionSummary(
  executionId: string,
  prompt: string,
  result: string | null,
  durationMs: number,
  status: 'success' | 'error',
): string {
  return JSON.stringify({
    executionId,
    promptPreview: prompt.slice(0, 200),
    resultPreview: result?.slice(0, 200) || null,
    durationMs,
    status,
    timestamp: new Date().toISOString(),
  });
}
