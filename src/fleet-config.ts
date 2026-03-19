/**
 * Fleet Configuration for AutomatanAbot
 * Handles AMS Fleet mode settings
 */

import { logger } from './logger.js';

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
  const enabled = process.env.AMS_FLEET_ENABLED === 'true' || !!process.env.AMS_URL;

  const config: FleetConfig = {
    enabled,
    amsEndpoint: process.env.AMS_URL || '',
    agentId: process.env.AMS_AGENT_ID || process.env.AGENT_ID || '',
    tenantId: process.env.AMS_TENANT_ID || process.env.TENANT_ID || '',
    agentToken: process.env.AMS_AGENT_TOKEN || process.env.AGENT_TOKEN || '',
    heartbeatIntervalMs: parseInt(process.env.AMS_HEARTBEAT_INTERVAL_MS || '30000', 10),
    contextThresholdPercent: parseInt(process.env.AMS_CONTEXT_THRESHOLD_PERCENT || '85', 10),
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
