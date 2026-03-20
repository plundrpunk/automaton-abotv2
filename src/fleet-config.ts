/**
 * Fleet Configuration for AutomatonAbot
 * Handles AMS Fleet mode settings
 */

import { readEnvFile } from './env.js';
import { logger } from './logger.js';

const envConfig = readEnvFile([
  'AMS_URL',
  'AMS_FLEET_ENABLED',
  'AMS_AGENT_ID',
  'AMS_TENANT_ID',
  'AMS_AGENT_TOKEN',
  'AMS_HEARTBEAT_INTERVAL_MS',
  'AMS_CONTEXT_THRESHOLD_PERCENT',
  'AMS_INSTANCE_ID',
  'AMS_GATEWAY_URL',
]);

function env(key: string): string {
  return process.env[key] || envConfig[key] || '';
}

export interface FleetConfig {
  enabled: boolean;
  amsEndpoint: string;
  agentId: string;
  tenantId: string;
  agentToken: string;
  heartbeatIntervalMs: number;
  contextThresholdPercent: number;
}

/**
 * Load Fleet configuration from environment
 */
export function loadFleetConfig(): FleetConfig {
  const enabled = env('AMS_FLEET_ENABLED') === 'true' || !!env('AMS_URL');

  const config: FleetConfig = {
    enabled,
    amsEndpoint: env('AMS_URL'),
    agentId: env('AMS_AGENT_ID') || env('AGENT_ID'),
    tenantId: env('AMS_TENANT_ID') || env('TENANT_ID'),
    agentToken: env('AMS_AGENT_TOKEN') || env('AGENT_TOKEN'),
    heartbeatIntervalMs: parseInt(env('AMS_HEARTBEAT_INTERVAL_MS') || '30000', 10),
    contextThresholdPercent: parseInt(env('AMS_CONTEXT_THRESHOLD_PERCENT') || '85', 10),
  };

  if (enabled) {
    if (!config.agentId || !config.tenantId || !config.agentToken) {
      logger.warn('AMS Fleet enabled but missing required config (agentId, tenantId, or agentToken)');
      config.enabled = false;
    } else {
      logger.info(
        { amsEndpoint: config.amsEndpoint, agentId: config.agentId },
        'AMS Fleet mode enabled',
      );
    }
  }

  return config;
}

/**
 * Validate Fleet configuration
 */
export function validateFleetConfig(config: FleetConfig): string[] {
  const errors: string[] = [];

  if (config.enabled) {
    if (!config.amsEndpoint) errors.push('AMS_URL is required');
    if (!config.agentId) errors.push('AMS_AGENT_ID is required');
    if (!config.tenantId) errors.push('AMS_TENANT_ID is required');
    if (!config.agentToken) errors.push('AMS_AGENT_TOKEN is required');

    if (config.heartbeatIntervalMs < 5000) {
      errors.push('AMS_HEARTBEAT_INTERVAL_MS must be >= 5000ms');
    }

    if (config.contextThresholdPercent < 50 || config.contextThresholdPercent > 100) {
      errors.push('AMS_CONTEXT_THRESHOLD_PERCENT must be between 50 and 100');
    }
  }

  return errors;
}

// Export singleton config
let fleetConfig: FleetConfig | null = null;

export function getFleetConfig(): FleetConfig {
  if (!fleetConfig) {
    fleetConfig = loadFleetConfig();
  }
  return fleetConfig;
}

export function isFleetMode(): boolean {
  return getFleetConfig().enabled;
}
