# AutomatonAbot v2

Open-source AI agent orchestrator with persistent memory, container pooling, and multi-channel messaging.

## What is AutomatonAbot?

AutomatonAbot is a daemon that manages persistent Claude Code agent containers. Each agent runs in an isolated Docker container with its own session context, tools, and memory. Agents communicate with users through the AMS Communications Gateway, which supports Telegram, Discord, Slack, and WhatsApp.

## Architecture

```
Channels (Telegram/Discord/Slack/WhatsApp)
    ↓
AMS Communications Gateway (port 18800)
    ↓
AutomatonAbot Host Daemon
    ↓
Docker Container Pool
    ├── Agent 1 (Claude Code + MCP tools)
    ├── Agent 2 (Claude Code + MCP tools)
    └── Agent N ...
    ↓
AMS (Persistent Memory + Fleet Management)
```

## Features

- **Persistent Container Pool** — Containers stay alive between messages, accumulating session context
- **Death/Birth Rituals** — When context fills up (85%), the agent crystallizes memories and spawns a successor
- **Multi-Channel Messaging** — Receive messages from any channel via AMS Communications Gateway
- **Persistent Memory** — All agents share access to AMS hierarchical memory (episodic, semantic, procedural)
- **Scheduled Tasks** — Cron, interval, and one-shot task scheduling with full agent capabilities
- **Mount Security** — Tamper-proof allowlist controls what host directories agents can access
- **Fleet Telemetry** — Real-time heartbeats, execution streaming, and Observatory dashboard integration

## Prerequisites

- Docker
- Node.js >= 20
- An AMS instance (for persistent memory and communications gateway)

## Quick Start

1. Clone the repository:
   ```bash
   git clone https://github.com/automaton/automaton-abotv2.git
   cd automaton-abotv2
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Build the agent container image:
   ```bash
   npm run docker:build
   ```

4. Configure your environment:
   ```bash
   cp .env.example .env
   # Edit .env with your AMS credentials
   ```

5. Start the daemon:
   ```bash
   npm run dev
   ```

## Configuration

See `.env.example` for all available configuration options.

### Required Environment Variables

| Variable | Description |
|----------|-------------|
| `AMS_URL` | URL of your AMS instance |
| `AMS_AGENT_ID` | Agent ID from AMS dashboard |
| `AMS_TENANT_ID` | Tenant ID from AMS dashboard |
| `AMS_AGENT_TOKEN` | Agent authentication token |

### Optional Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ASSISTANT_NAME` | `Abot` | Agent trigger name |
| `MAX_CONCURRENT_CONTAINERS` | `5` | Max simultaneous agent containers |
| `IDLE_TIMEOUT` | `1800000` | Container idle timeout (ms) |
| `CONTAINER_IMAGE` | `automaton-agent:latest` | Docker image for agents |
| `LOG_LEVEL` | `info` | Logging level |

## Development

```bash
# Type check
npm run typecheck

# Run tests
npm test

# Format code
npm run format

# Build
npm run build
```

## License

MIT
