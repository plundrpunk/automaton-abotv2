/**
 * Container Pool for AutomatanAbot
 *
 * Manages persistent container sessions per group.
 * First message goes via stdin. Follow-ups go via IPC files.
 * Agent-runner inside the container already polls /workspace/ipc/input/
 * for follow-up messages and supports the _close sentinel for graceful shutdown.
 *
 * This converts the ephemeral-per-message model into a daemon-style pattern
 * where containers stay alive between messages, accumulating Claude Code
 * session context until the Warden triggers a death ritual.
 */
import { ChildProcess, exec } from 'child_process';
import { EventEmitter } from 'events';
import fs from 'fs';
import path from 'path';

import { DATA_DIR, IDLE_TIMEOUT } from './config.js';
import { buildStopContainerCommand } from './container-runtime.js';
import { ContainerOutput } from './container-runner.js';
import { logger } from './logger.js';

const OUTPUT_START_MARKER = '---AUTOMATON_OUTPUT_START---';
const OUTPUT_END_MARKER = '---AUTOMATON_OUTPUT_END---';

interface PoolEntry {
  containerName: string;
  process: ChildProcess;
  sessionId?: string;
  groupFolder: string;
  ipcInputDir: string;
  lastActivity: number;
  spawnedAt: number;
  alive: boolean;
  events: EventEmitter;
  outputChain: Promise<void>;
  onOutput?: (output: ContainerOutput) => Promise<void>;
}

export class ContainerPool {
  private pool: Map<string, PoolEntry> = new Map();
  private reaperInterval: NodeJS.Timeout | null = null;

  constructor(private idleTimeoutMs: number = IDLE_TIMEOUT) {
    this.startReaper();
  }

  has(groupFolder: string): boolean {
    const entry = this.pool.get(groupFolder);
    return !!entry && entry.alive;
  }

  getSessionId(groupFolder: string): string | undefined {
    return this.pool.get(groupFolder)?.sessionId;
  }

  register(
    groupFolder: string,
    containerName: string,
    proc: ChildProcess,
    onOutput?: (output: ContainerOutput) => Promise<void>,
  ): PoolEntry {
    const ipcInputDir = path.join(DATA_DIR, 'ipc', groupFolder, 'input');
    fs.mkdirSync(ipcInputDir, { recursive: true });

    const entry: PoolEntry = {
      containerName,
      process: proc,
      groupFolder,
      ipcInputDir,
      lastActivity: Date.now(),
      spawnedAt: Date.now(),
      alive: true,
      events: new EventEmitter(),
      outputChain: Promise.resolve(),
      onOutput,
    };

    let stderrBuf = '';

    this.attachOutputParser(entry);

    proc.stderr?.on('data', (data: Buffer) => {
      stderrBuf += data.toString();
    });

    proc.on('close', (code) => {
      entry.alive = false;
      logger.info(
        { group: groupFolder, containerName, code, stderr: stderrBuf.slice(-1000) || undefined },
        'Pooled container exited',
      );
      entry.events.emit('closed', code);
      this.pool.delete(groupFolder);
    });

    proc.on('error', (err) => {
      entry.alive = false;
      logger.error(
        { group: groupFolder, containerName, error: err },
        'Pooled container process error',
      );
      entry.events.emit('error', err);
      this.pool.delete(groupFolder);
    });

    this.pool.set(groupFolder, entry);

    logger.info(
      { group: groupFolder, containerName },
      'Container registered in pool',
    );

    return entry;
  }

  async sendMessage(
    groupFolder: string,
    prompt: string,
    onOutput?: (output: ContainerOutput) => Promise<void>,
  ): Promise<ContainerOutput> {
    const entry = this.pool.get(groupFolder);
    if (!entry || !entry.alive) {
      throw new Error(`No active container for group ${groupFolder}`);
    }

    entry.lastActivity = Date.now();

    if (onOutput) {
      entry.onOutput = onOutput;
    }

    const filename = `msg-${Date.now()}.json`;
    const filepath = path.join(entry.ipcInputDir, filename);
    fs.writeFileSync(filepath, JSON.stringify({ type: 'message', text: prompt }));

    logger.info(
      { group: groupFolder, filename, promptLength: prompt.length },
      'IPC message sent to pooled container',
    );

    return this.waitForOutput(entry);
  }

  waitForOutput(entry: PoolEntry): Promise<ContainerOutput> {
    return new Promise<ContainerOutput>((resolve) => {
      const onOutput = (output: ContainerOutput) => {
        cleanup();
        resolve(output);
      };

      const onClosed = (code: number) => {
        cleanup();
        resolve({
          status: code === 0 ? 'success' : 'error',
          result: null,
          newSessionId: entry.sessionId,
          error: code !== 0
            ? `Container exited with code ${code} while waiting for output`
            : undefined,
        });
      };

      const onError = (err: Error) => {
        cleanup();
        resolve({
          status: 'error',
          result: null,
          error: `Container error: ${err.message}`,
        });
      };

      const cleanup = () => {
        entry.events.removeListener('output', onOutput);
        entry.events.removeListener('closed', onClosed);
        entry.events.removeListener('error', onError);
      };

      entry.events.once('output', onOutput);
      entry.events.once('closed', onClosed);
      entry.events.once('error', onError);
    });
  }

  async teardown(groupFolder: string): Promise<void> {
    const entry = this.pool.get(groupFolder);
    if (!entry) return;

    const sentinel = path.join(entry.ipcInputDir, '_close');
    fs.writeFileSync(sentinel, '');

    logger.info(
      { group: groupFolder, containerName: entry.containerName },
      'Close sentinel written, awaiting graceful exit',
    );

    await new Promise<void>((resolve) => {
      if (!entry.alive) {
        resolve();
        return;
      }

      const timeout = setTimeout(() => {
        if (entry.alive) {
          logger.warn(
            { group: groupFolder },
            'Container did not exit after _close, force stopping',
          );
          exec(
            buildStopContainerCommand(entry.containerName),
            { timeout: 15_000 },
            () => {
              entry.process.kill('SIGKILL');
              resolve();
            },
          );
        } else {
          resolve();
        }
      }, 30_000);

      entry.events.once('closed', () => {
        clearTimeout(timeout);
        resolve();
      });
    });

    this.pool.delete(groupFolder);
  }

  async teardownAll(): Promise<void> {
    const groups = [...this.pool.keys()];
    logger.info({ count: groups.length }, 'Tearing down all pooled containers');
    await Promise.all(groups.map((g) => this.teardown(g)));
    if (this.reaperInterval) {
      clearInterval(this.reaperInterval);
      this.reaperInterval = null;
    }
  }

  status(): Array<{
    group: string;
    container: string;
    alive: boolean;
    sessionId?: string;
    ageMs: number;
    idleMs: number;
  }> {
    const now = Date.now();
    return [...this.pool.entries()].map(([group, entry]) => ({
      group,
      container: entry.containerName,
      alive: entry.alive,
      sessionId: entry.sessionId,
      ageMs: now - entry.spawnedAt,
      idleMs: now - entry.lastActivity,
    }));
  }

  private attachOutputParser(entry: PoolEntry): void {
    let parseBuffer = '';

    entry.process.stdout?.on('data', (data: Buffer) => {
      const chunk = data.toString();
      parseBuffer += chunk;

      let startIdx: number;
      while ((startIdx = parseBuffer.indexOf(OUTPUT_START_MARKER)) !== -1) {
        const endIdx = parseBuffer.indexOf(OUTPUT_END_MARKER, startIdx);
        if (endIdx === -1) break;

        const jsonStr = parseBuffer
          .slice(startIdx + OUTPUT_START_MARKER.length, endIdx)
          .trim();
        parseBuffer = parseBuffer.slice(endIdx + OUTPUT_END_MARKER.length);

        try {
          const parsed: ContainerOutput = JSON.parse(jsonStr);

          if (parsed.newSessionId) {
            entry.sessionId = parsed.newSessionId;
          }
          entry.lastActivity = Date.now();

          if (entry.onOutput) {
            entry.outputChain = entry.outputChain.then(() =>
              entry.onOutput!(parsed),
            );
          }

          entry.events.emit('output', parsed);
        } catch (err) {
          logger.warn(
            { group: entry.groupFolder, error: err },
            'Failed to parse pooled output chunk',
          );
        }
      }
    });

    entry.process.stderr?.on('data', (data: Buffer) => {
      const lines = data.toString().trim().split('\n');
      for (const line of lines) {
        if (line) logger.debug({ container: entry.groupFolder }, line);
      }
    });
  }

  private startReaper(): void {
    this.reaperInterval = setInterval(() => {
      const now = Date.now();
      for (const [group, entry] of this.pool) {
        if (entry.alive && now - entry.lastActivity > this.idleTimeoutMs) {
          logger.info(
            { group, idleMs: now - entry.lastActivity },
            'Reaping idle pooled container',
          );
          this.teardown(group).catch((err) => {
            logger.error(
              { group, error: err },
              'Failed to reap idle container',
            );
          });
        }
      }
    }, 30_000);
  }
}

/** Singleton pool instance */
export const containerPool = new ContainerPool();
