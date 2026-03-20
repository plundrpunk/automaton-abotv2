/**
 * Simple Health Server for AutomatonAbot
 * Provides /health endpoint for container health checks
 */

import http from 'http';
import { logger } from './logger.js';
import { getWardenState } from './embedded-warden.js';
import { getAMSClient } from './ams-client.js';
import { isFleetMode } from './fleet-config.js';

interface HealthStatus {
  ok: boolean;
  timestamp: string;
  uptime: number;
  fleet: {
    enabled: boolean;
    connected: boolean;
  };
  warden: {
    contextPercent: number;
    warned: boolean;
    totalRituals: number;
  };
}

let server: http.Server | null = null;
const startTime = Date.now();

export function startHealthServer(port: number = 8080): void {
  if (server) {
    logger.warn('Health server already running');
    return;
  }

  server = http.createServer(async (req, res) => {
    if (req.method === 'GET' && req.url === '/health') {
      const status = await getHealthStatus();

      res.writeHead(status.ok ? 200 : 503, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(status, null, 2));
    } else if (req.method === 'GET' && req.url === '/ready') {
      const ready = isFleetMode() ? !!getAMSClient() : true;
      res.writeHead(ready ? 200 : 503, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ready }));
    } else {
      res.writeHead(404);
      res.end('Not Found');
    }
  });

  server.listen(port, () => {
    logger.info({ port }, 'Health server started');
  });

  server.on('error', (err) => {
    logger.error({ err }, 'Health server error');
  });
}

export function stopHealthServer(): void {
  if (server) {
    server.close();
    server = null;
    logger.info('Health server stopped');
  }
}

async function getHealthStatus(): Promise<HealthStatus> {
  const wardenState = getWardenState();
  const client = getAMSClient();

  let amsConnected = false;
  if (client) {
    try {
      amsConnected = await client.healthCheck();
    } catch {
      amsConnected = false;
    }
  }

  const status: HealthStatus = {
    ok: true,
    timestamp: new Date().toISOString(),
    uptime: Math.floor((Date.now() - startTime) / 1000),
    fleet: {
      enabled: isFleetMode(),
      connected: amsConnected,
    },
    warden: {
      contextPercent: wardenState.contextPercent,
      warned: wardenState.warned,
      totalRituals: wardenState.totalRituals,
    },
  };

  if (wardenState.ritualInProgress || wardenState.contextPercent > 95) {
    status.ok = false;
  }

  return status;
}
