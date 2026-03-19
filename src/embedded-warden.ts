/**
 * Embedded Warden for AutomatanAbot
 * Monitors context usage and triggers death rituals
 */

import fs from 'fs';

import { logger } from './logger.js';
import { getAMSClient } from './ams-client.js';
import { getFleetConfig } from './fleet-config.js';
import { setContextUsage } from './phone-home.js';

interface WardenState {
  contextPercent: number;
  warned: boolean;
  ritualInProgress: boolean;
  lastRitualTime: number | null;
  totalRituals: number;
}

const state: WardenState = {
  contextPercent: 0,
  warned: false,
  ritualInProgress: false,
  lastRitualTime: null,
  totalRituals: 0,
};

let monitorInterval: ReturnType<typeof setInterval> | null = null;

export function startEmbeddedWarden(intervalMs: number = 10000): void {
  const config = getFleetConfig();
  if (!config.enabled) {
    logger.debug('Fleet mode disabled, skipping embedded warden');
    return;
  }

  logger.info({ intervalMs, threshold: config.contextThresholdPercent }, 'Starting embedded warden');

  monitorInterval = setInterval(() => {
    checkThresholds(config.contextThresholdPercent);
  }, intervalMs);
}

export function stopEmbeddedWarden(): void {
  if (monitorInterval) {
    clearInterval(monitorInterval);
    monitorInterval = null;
    logger.info('Stopped embedded warden');
  }
}

export function updateContextUsage(percent: number): void {
  state.contextPercent = percent;
  setContextUsage(percent);
}

function checkThresholds(criticalThreshold: number): void {
  const warnThreshold = criticalThreshold - 10;

  if (state.ritualInProgress) {
    return;
  }

  if (state.contextPercent >= criticalThreshold) {
    logger.warn(
      { contextPercent: state.contextPercent, threshold: criticalThreshold },
      'Context threshold breached - triggering death ritual',
    );
    triggerDeathRitual();
    return;
  }

  if (state.contextPercent >= warnThreshold && !state.warned) {
    logger.warn(
      { contextPercent: state.contextPercent, threshold: warnThreshold },
      'Context approaching threshold - consider wrapping up',
    );
    state.warned = true;
  } else if (state.contextPercent < warnThreshold) {
    state.warned = false;
  }
}

async function triggerDeathRitual(): Promise<void> {
  const client = getAMSClient();
  if (!client) {
    logger.error('Cannot perform death ritual - AMS client not initialized');
    return;
  }

  state.ritualInProgress = true;

  try {
    logger.info('Starting death ritual...');

    const sessionMemoryId = await client.createMemory({
      title: `Session Summary - ${new Date().toISOString().split('T')[0]}`,
      content: `Abot session ended at ${state.contextPercent}% context usage. Ritual triggered by embedded warden.`,
      memoryTier: 'episodic',
      entityType: 'event',
      importance: 0.6,
      tags: ['death-ritual', 'auto-generated'],
    });

    if (sessionMemoryId) {
      logger.info({ memoryId: sessionMemoryId }, 'Created session memory');
    }

    const continuationId = await client.createContinuation({
      originalGoal: 'Continue session work',
      nextAction: 'Claim this continuation and resume where predecessor left off',
      contextUsageAtDeath: state.contextPercent,
      handoffNotes: 'Auto-generated continuation from embedded warden death ritual',
    });

    if (continuationId) {
      logger.info({ continuationId }, 'Created continuation');
    }

    state.totalRituals += 1;
    state.lastRitualTime = Date.now();
    state.warned = false;

    logger.info(
      { totalRituals: state.totalRituals },
      'Death ritual complete - requesting restart',
    );

    signalRestartNeeded();

  } catch (err) {
    logger.error({ err }, 'Death ritual failed');
  } finally {
    state.ritualInProgress = false;
  }
}

function signalRestartNeeded(): void {
  const restartMarker = '/workspace/ipc/.restart-requested';

  try {
    fs.writeFileSync(restartMarker, JSON.stringify({
      reason: 'death_ritual',
      timestamp: new Date().toISOString(),
      contextPercent: state.contextPercent,
    }));
    logger.info('Restart marker written');

    setTimeout(() => {
      process.exit(0);
    }, 1000);
  } catch (err) {
    logger.error({ err }, 'Failed to write restart marker');
  }
}

export async function performBirthRitual(): Promise<void> {
  const client = getAMSClient();
  if (!client) {
    logger.debug('AMS client not initialized, skipping birth ritual');
    return;
  }

  logger.info('Performing birth ritual...');

  try {
    const restartMarker = '/workspace/ipc/.restart-requested';

    if (fs.existsSync(restartMarker)) {
      const data = JSON.parse(fs.readFileSync(restartMarker, 'utf8'));
      logger.info({ previousContext: data.contextPercent }, 'Resuming from death ritual');
      fs.unlinkSync(restartMarker);
    }

    logger.info('Birth ritual complete - ready to work');

  } catch (err) {
    logger.error({ err }, 'Birth ritual error');
  }
}

export function getWardenState(): WardenState {
  return { ...state };
}
