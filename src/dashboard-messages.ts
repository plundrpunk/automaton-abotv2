import { AgentMessage } from './ams-client.js';
import { ASSISTANT_NAME } from './config.js';
import { ContainerOutput } from './container-runner.js';
import { logger } from './logger.js';
import { RegisteredGroup } from './types.js';

type DashboardRunAgent = (
  group: RegisteredGroup,
  prompt: string,
  chatJid: string,
  onOutput?: (output: ContainerOutput) => Promise<void>,
) => Promise<unknown>;

export function dispatchDashboardMessages(
  messages: AgentMessage[],
  runAgentFn: DashboardRunAgent,
): void {
  for (const msg of messages) {
    logger.info(
      { type: msg.type, sender: msg.sender, content: msg.content.slice(0, 100) },
      'Dashboard message received',
    );

    if (!msg.content?.trim()) {
      continue;
    }

    const dashboardGroup: RegisteredGroup = {
      name: ASSISTANT_NAME,
      folder: 'dashboard',
      trigger: '@Dashboard',
      added_at: new Date().toISOString(),
      requiresTrigger: false,
    };

    void runAgentFn(dashboardGroup, msg.content, '__dashboard__', async (result) => {
      if (result.result) {
        const raw = typeof result.result === 'string' ? result.result : JSON.stringify(result.result);
        const text = raw.replace(/<internal>[\s\S]*?<\/internal>/g, '').trim();
        logger.info({ content: text.slice(0, 200) }, 'Dashboard agent response');
      }
    }).catch((err) => logger.error({ err }, 'Dashboard agent error'));
  }
}
