import { execSync } from 'child_process';

export type ContainerRuntime = 'docker';

type ListedContainer = {
  id: string;
  status: string;
};

export function getContainerRuntime(): ContainerRuntime {
  return 'docker';
}

export function getContainerCommand(): string {
  return 'docker';
}

export function getContainerRuntimeLabel(): string {
  return 'Docker';
}

export function buildMountArgs(
  hostPath: string,
  containerPath: string,
  readonly: boolean,
): string[] {
  if (!readonly) {
    return ['-v', `${hostPath}:${containerPath}`];
  }
  return ['-v', `${hostPath}:${containerPath}:ro`];
}

export function buildStopContainerCommand(containerName: string): string {
  return `docker stop ${containerName}`;
}

export function ensureContainerRuntimeReady(): void {
  execSync('docker info', { stdio: 'pipe' });
}

export function listManagedContainers(): ListedContainer[] {
  const output = execSync(`docker ps -a --format '{{json .}}'`, {
    stdio: ['pipe', 'pipe', 'pipe'],
    encoding: 'utf-8',
  });

  return output
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line) as { Names?: string; Status?: string })
    .map((entry) => ({
      id: entry.Names || '',
      status: (entry.Status || '').toLowerCase(),
    }))
    .filter((entry) => entry.id);
}
