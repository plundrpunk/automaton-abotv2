/**
 * Container Runner for AutomatonAbot
 * Spawns agent execution in Docker containers and handles IPC
 */
import { ChildProcess, exec, spawn } from 'child_process';
import fs from 'fs';
import os from 'os';
import path from 'path';

import {
  CONTAINER_IMAGE,
  CONTAINER_MAX_OUTPUT_SIZE,
  CONTAINER_TIMEOUT,
  DATA_DIR,
  GROUPS_DIR,
  IDLE_TIMEOUT,
} from './config.js';
import {
  buildMountArgs,
  buildStopContainerCommand,
  getContainerCommand,
  getContainerRuntimeLabel,
} from './container-runtime.js';
import { readEnvFile } from './env.js';
import { logger } from './logger.js';
import { validateAdditionalMounts } from './mount-security.js';
import { containerPool } from './container-pool.js';
import { RegisteredGroup } from './types.js';

// Sentinel markers for robust output parsing (must match agent-runner)
const OUTPUT_START_MARKER = '---AUTOMATON_OUTPUT_START---';
const OUTPUT_END_MARKER = '---AUTOMATON_OUTPUT_END---';

function getHomeDir(): string {
  const home = process.env.HOME || os.homedir();
  if (!home) {
    throw new Error(
      'Unable to determine home directory: HOME environment variable is not set and os.homedir() returned empty',
    );
  }
  return home;
}

export interface ContainerInput {
  prompt: string;
  sessionId?: string;
  groupFolder: string;
  chatJid: string;
  isMain: boolean;
  isScheduledTask?: boolean;
  secrets?: Record<string, string>;
}

export interface ContainerOutput {
  status: 'success' | 'error';
  result: string | null;
  newSessionId?: string;
  error?: string;
}

interface VolumeMount {
  hostPath: string;
  containerPath: string;
  readonly: boolean;
}

function buildVolumeMounts(
  group: RegisteredGroup,
  isMain: boolean,
): VolumeMount[] {
  const mounts: VolumeMount[] = [];
  const homeDir = getHomeDir();
  const projectRoot = process.cwd();

  if (isMain) {
    mounts.push({
      hostPath: projectRoot,
      containerPath: '/workspace/project',
      readonly: false,
    });

    mounts.push({
      hostPath: path.join(GROUPS_DIR, group.folder),
      containerPath: '/workspace/group',
      readonly: false,
    });
  } else {
    mounts.push({
      hostPath: path.join(GROUPS_DIR, group.folder),
      containerPath: '/workspace/group',
      readonly: false,
    });

    const globalDir = path.join(GROUPS_DIR, 'global');
    if (fs.existsSync(globalDir)) {
      mounts.push({
        hostPath: globalDir,
        containerPath: '/workspace/global',
        readonly: true,
      });
    }
  }

  // Per-group Claude sessions directory
  const groupSessionsDir = path.join(
    DATA_DIR,
    'sessions',
    group.folder,
    '.claude',
  );
  fs.mkdirSync(groupSessionsDir, { recursive: true });
  const settingsFile = path.join(groupSessionsDir, 'settings.json');
  if (!fs.existsSync(settingsFile)) {
    fs.writeFileSync(settingsFile, JSON.stringify({
      env: {
        CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: '1',
        CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD: '1',
        CLAUDE_CODE_DISABLE_AUTO_MEMORY: '0',
      },
    }, null, 2) + '\n');
  }

  const hostClaudeCredentials = path.join(getHomeDir(), '.claude', '.credentials.json');
  if (fs.existsSync(hostClaudeCredentials)) {
    fs.copyFileSync(
      hostClaudeCredentials,
      path.join(groupSessionsDir, '.credentials.json'),
    );
  }

  // Sync skills from container/skills/ into each group's .claude/skills/
  const skillsSrc = path.join(process.cwd(), 'container', 'skills');
  const skillsDst = path.join(groupSessionsDir, 'skills');
  if (fs.existsSync(skillsSrc)) {
    for (const skillDir of fs.readdirSync(skillsSrc)) {
      const srcDir = path.join(skillsSrc, skillDir);
      if (!fs.statSync(srcDir).isDirectory()) continue;
      const dstDir = path.join(skillsDst, skillDir);
      fs.mkdirSync(dstDir, { recursive: true });
      for (const file of fs.readdirSync(srcDir)) {
        const srcFile = path.join(srcDir, file);
        const dstFile = path.join(dstDir, file);
        fs.copyFileSync(srcFile, dstFile);
      }
    }
  }
  mounts.push({
    hostPath: groupSessionsDir,
    containerPath: '/home/node/.claude',
    readonly: false,
  });

  // Per-group IPC namespace
  const groupIpcDir = path.join(DATA_DIR, 'ipc', group.folder);
  fs.mkdirSync(path.join(groupIpcDir, 'messages'), { recursive: true });
  fs.mkdirSync(path.join(groupIpcDir, 'tasks'), { recursive: true });
  fs.mkdirSync(path.join(groupIpcDir, 'input'), { recursive: true });
  mounts.push({
    hostPath: groupIpcDir,
    containerPath: '/workspace/ipc',
    readonly: false,
  });

  // Mount agent-runner source from host — recompiled on container startup
  const agentRunnerSrc = path.join(projectRoot, 'container', 'agent-runner', 'src');
  mounts.push({
    hostPath: agentRunnerSrc,
    containerPath: '/app/src',
    readonly: true,
  });

  // Additional mounts validated against external allowlist
  if (group.containerConfig?.additionalMounts) {
    const validatedMounts = validateAdditionalMounts(
      group.containerConfig.additionalMounts,
      group.name,
      isMain,
    );
    mounts.push(...validatedMounts);
  }

  return mounts;
}

/**
 * Read allowed secrets from .env for passing to the container via stdin.
 * Secrets are never written to disk or mounted as files.
 */
function readSecrets(): Record<string, string> {
  const keys = ['ANTHROPIC_API_KEY'];
  const hostClaudeCredentials = path.join(getHomeDir(), '.claude', '.credentials.json');
  if (!fs.existsSync(hostClaudeCredentials)) {
    keys.push('CLAUDE_CODE_OAUTH_TOKEN');
  }
  return readEnvFile(keys);
}

function buildContainerArgs(mounts: VolumeMount[], containerName: string): string[] {
  const args: string[] = ['run', '-i', '--name', containerName];

  // Run as host user so bind-mounted files are accessible
  const hostUid = process.getuid?.();
  const hostGid = process.getgid?.();
  if (hostUid != null && hostUid !== 0 && hostUid !== 1000) {
    args.push('--user', `${hostUid}:${hostGid}`);
    args.push('-e', 'HOME=/home/node');
  }

  // Docker networking: join the AMS network so containers can reach AMS services
  const amsNetwork = process.env.AMS_DOCKER_NETWORK || 'ams_ams_network';
  args.push('--network', amsNetwork);
  // Linux Docker host access (host.docker.internal doesn't exist by default on Linux)
  args.push('--add-host=host.docker.internal:host-gateway');

  // Inside containers, use Docker service names to reach AMS stack.
  // The host daemon uses localhost, but containers need Docker DNS names.
  const containerAmsUrl = process.env.AMS_CONTAINER_URL || 'http://ams-server:3001';
  const containerMcpEndpoint = process.env.AMS_CONTAINER_MCP_ENDPOINT || 'http://ams-mcp-sse:3002/sse';
  args.push('-e', `AMS_URL=${containerAmsUrl}`);
  args.push('-e', `AMS_MCP_ENDPOINT=${containerMcpEndpoint}`);

  if (process.env.AMS_AGENT_ID) {
    args.push('-e', `AMS_AGENT_ID=${process.env.AMS_AGENT_ID}`);
  }
  if (process.env.AMS_TENANT_ID) {
    args.push('-e', `AMS_TENANT_ID=${process.env.AMS_TENANT_ID}`);
  }
  if (process.env.AMS_AGENT_TOKEN) {
    args.push('-e', `AMS_AGENT_TOKEN=${process.env.AMS_AGENT_TOKEN}`);
  }
  if (process.env.AMS_TOOL_MODE) {
    args.push('-e', `AMS_TOOL_MODE=${process.env.AMS_TOOL_MODE}`);
  }

  for (const mount of mounts) {
    args.push(...buildMountArgs(mount.hostPath, mount.containerPath, mount.readonly));
  }

  args.push(CONTAINER_IMAGE);

  return args;
}


/**
 * Spawn a new persistent container and register it in the pool.
 * First message goes via stdin; follow-ups will go via IPC files.
 * Returns when the first output marker pair is received.
 */
async function spawnPooledContainer(
  group: RegisteredGroup,
  input: ContainerInput,
  onProcess: (proc: ChildProcess, containerName: string) => void,
  onOutput?: (output: ContainerOutput) => Promise<void>,
): Promise<ContainerOutput> {
  const groupDir = path.join(GROUPS_DIR, group.folder);
  fs.mkdirSync(groupDir, { recursive: true });

  const mounts = buildVolumeMounts(group, input.isMain);
  const safeName = group.folder.replace(/[^a-zA-Z0-9-]/g, '-');
  const containerName = `automaton-${safeName}-${Date.now()}`;
  const containerArgs = buildContainerArgs(mounts, containerName);
  logger.info(
    { group: group.name, runtime: getContainerRuntimeLabel(), containerName, containerArgs },
    'Spawning pooled container command',
  );

  const container = spawn(getContainerCommand(), containerArgs, {
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  // Register in pool
  const entry = containerPool.register(
    group.folder,
    containerName,
    container,
    onOutput,
  );

  onProcess(container, containerName);

  // Send first message via stdin (secrets included, then scrubbed)
  input.secrets = readSecrets();
  container.stdin!.write(JSON.stringify(input));
  container.stdin!.end();
  delete input.secrets;

  // Wait for the first output marker from the agent-runner
  return containerPool.waitForOutput(entry);
}

export async function runContainerAgent(
  group: RegisteredGroup,
  input: ContainerInput,
  onProcess: (proc: ChildProcess, containerName: string) => void,
  onOutput?: (output: ContainerOutput) => Promise<void>,
): Promise<ContainerOutput> {
  // Reuse: if a container is already alive for this group, send via IPC
  if (containerPool.has(group.folder)) {
    return containerPool.sendMessage(group.folder, input.prompt, onOutput);
  }

  // Spawn new pooled container
  return spawnPooledContainer(group, input, onProcess, onOutput);
}

export function writeTasksSnapshot(
  groupFolder: string,
  isMain: boolean,
  tasks: Array<{
    id: string;
    groupFolder: string;
    prompt: string;
    schedule_type: string;
    schedule_value: string;
    status: string;
    next_run: string | null;
  }>,
): void {
  const groupIpcDir = path.join(DATA_DIR, 'ipc', groupFolder);
  fs.mkdirSync(groupIpcDir, { recursive: true });

  const filteredTasks = isMain
    ? tasks
    : tasks.filter((t) => t.groupFolder === groupFolder);

  const tasksFile = path.join(groupIpcDir, 'current_tasks.json');
  fs.writeFileSync(tasksFile, JSON.stringify(filteredTasks, null, 2));
}

export interface AvailableGroup {
  jid: string;
  name: string;
  lastActivity: string;
  isRegistered: boolean;
}

export function writeGroupsSnapshot(
  groupFolder: string,
  isMain: boolean,
  groups: AvailableGroup[],
  registeredJids: Set<string>,
): void {
  const groupIpcDir = path.join(DATA_DIR, 'ipc', groupFolder);
  fs.mkdirSync(groupIpcDir, { recursive: true });

  const visibleGroups = isMain ? groups : [];

  const groupsFile = path.join(groupIpcDir, 'available_groups.json');
  fs.writeFileSync(
    groupsFile,
    JSON.stringify(
      {
        groups: visibleGroups,
        lastSync: new Date().toISOString(),
      },
      null,
      2,
    ),
  );
}
