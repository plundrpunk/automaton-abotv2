<div align="center">

# ⚡ Automaton Abot

**AI agents that remember everything.**

Open-source agent orchestrator with persistent memory, lifecycle management, and multi-channel messaging — powered by the [Automaton Memory System](https://automaton-memory.com).

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Node.js](https://img.shields.io/badge/node-%3E%3D20-brightgreen.svg)](https://nodejs.org)
[![TypeScript](https://img.shields.io/badge/typescript-5.x-blue.svg)](https://www.typescriptlang.org)
[![Docker](https://img.shields.io/badge/docker-required-blue.svg)](https://www.docker.com)

---

**Most AI agents forget everything between sessions.**
**Automaton Abots don't.**

</div>

## What is this?

Automaton Abot is a daemon that orchestrates persistent AI agent containers. Each agent runs in an isolated Docker environment with its own tools, context, and session history. When an agent's context window fills up, it doesn't just die — it **crystallizes its memories** into the Automaton Memory System and spawns a successor that picks up where it left off.

This is the **body**. The brain lives in [AMS](https://automaton-memory.com).

```
┌─────────────────────────────────────────────────────────┐
│                     Channels                            │
│         Telegram · Discord · Slack · WhatsApp           │
│              Dashboard · API · CLI                      │
└──────────────────────┬──────────────────────────────────┘
                       │
         ┌─────────────▼──────────────┐
         │   AMS Communications       │
         │       Gateway              │
         └─────────────┬──────────────┘
                       │
         ┌─────────────▼──────────────┐
         │   Automaton Abot Daemon    │
         │                            │
         │  ┌──────────────────────┐  │
         │  │   Container Pool     │  │
         │  │                      │  │
         │  │  ┌────┐ ┌────┐      │  │
         │  │  │ A1 │ │ A2 │ ...  │  │
         │  │  └──┬─┘ └──┬─┘      │  │
         │  └─────┼──────┼────────┘  │
         │        │      │           │
         │  ┌─────▼──────▼────────┐  │
         │  │  Embedded Warden    │  │
         │  │  (lifecycle mgmt)   │  │
         │  └─────────────────────┘  │
         └─────────────┬──────────────┘
                       │
         ┌─────────────▼──────────────┐
         │   Automaton Memory System  │
         │                            │
         │  Episodic · Semantic ·     │
         │  Procedural Memory         │
         │  Fleet · Observatory ·     │
         │  Automatons                │
         └────────────────────────────┘
```

## Why?

Every AI agent framework gives you stateless execution. You get a prompt in, a response out, and amnesia in between. Building anything that **learns**, **improves**, or **maintains context** across sessions requires bolting on your own memory layer.

Automaton Abot ships with that layer built in:

- **Persistent memory** across container restarts, context resets, and even server reboots
- **Death/birth rituals** — when context fills to 85%, the agent writes a session summary to memory, creates a continuation, and its successor claims it on boot
- **Hierarchical memory** — episodic (what happened), semantic (what things mean), procedural (how to do things)
- **Fleet coordination** — multiple agents sharing a memory system, visible in real-time via Observatory

## Features

| Feature | Description |
|---------|-------------|
| **Container Pool** | Persistent Docker sessions that survive between messages. No cold starts. |
| **Death/Birth Rituals** | Automatic memory crystallization at 85% context. Successors inherit knowledge. |
| **Multi-Channel** | Telegram, Discord, Slack, WhatsApp via AMS Communications Gateway. |
| **Persistent Memory** | Three-tier AMS memory: episodic, semantic, procedural. Survives everything. |
| **Task Scheduler** | Cron, interval, and one-shot tasks with full agent capabilities. |
| **Mount Security** | Tamper-proof allowlist for host directory access. No container escapes. |
| **Fleet Telemetry** | Real-time heartbeats, execution streaming, Observatory dashboard. |
| **Skills System** | Agents load SKILL.md files for domain-specific capabilities. |
| **MCP Bridge** | AMS tools exposed to agents via Model Context Protocol over SSE→stdio bridge. |

## Quick Start

### Prerequisites

- **Docker** — running and accessible
- **Node.js ≥ 20**
- **An AMS account** — sign up at [automaton-memory.com](https://automaton-memory.com) and create your first Abot through the guided interview

### 1. Clone & Install

```bash
git clone https://github.com/plundrpunk/automaton-abotv2.git
cd automaton-abotv2
npm install
```

### 2. Build the Agent Container

```bash
npm run docker:build
```

This creates the `automaton-agent:latest` image — an isolated runtime with Claude Code, agent-browser, and the MCP bridge pre-installed.

### 3. Configure

```bash
cp .env.example .env
```

Fill in your AMS credentials (you get these from the AMS dashboard after completing the guided interview):

```env
# Required — connects your Abot to its brain
AMS_URL=https://automaton-memory.com
AMS_AGENT_ID=your-agent-id
AMS_TENANT_ID=your-tenant-id
AMS_AGENT_TOKEN=your-agent-token
```

### 4. Start

```bash
# Development
npm run dev

# Production
npm run build && npm start
```

Your Abot will phone home to AMS, perform a birth ritual, and appear online in the Observatory dashboard.

### 5. Talk to It

Send a message through any connected channel (Telegram, Dashboard Chat, etc.) and watch it spawn a container, think, respond, and **remember**.

## How It Works

### The Body/Head Split

This repo is the **body** — the runtime, container management, fleet protocol, and daemon lifecycle. It's open source and runs anywhere Docker does.

The **head** — your agent's identity, personality, memories, and learned behaviors — lives in AMS. It's created through a guided interview on the AMS dashboard. When the body boots with valid AMS credentials, it phones home, finds its head, and they **marry**. The agent comes alive with persistent memory, identity, and the full capability stack.

### Container Lifecycle

```
Message arrives → Container spawns (or reuses from pool)
                     ↓
              Agent processes message
              (Claude Code + MCP tools + skills)
                     ↓
              Response streams back to channel
                     ↓
              Container stays alive in pool
              (accumulating session context)
                     ↓
              Context hits 85% → Warden triggers
                     ↓
              Death Ritual:
                • Session summary → episodic memory
                • Active tasks → continuation
                • State snapshot → AMS
                     ↓
              Container exits → Successor spawns
              → Claims continuation → Resumes work
```

### Phone Home Protocol

Every 30 seconds, the Abot sends a heartbeat to AMS with:
- Container health metrics (memory, CPU, uptime)
- Token usage since last heartbeat
- Active execution count
- Context window usage percentage

AMS uses this for fleet management, cost tracking, and the Observatory real-time dashboard.

## Project Structure

```
automaton-abotv2/
├── src/
│   ├── index.ts              # Main daemon entry point
│   ├── container-pool.ts     # Persistent container session manager
│   ├── container-runner.ts   # Container spawning and I/O
│   ├── embedded-warden.ts    # Context monitoring, death/birth rituals
│   ├── phone-home.ts         # Heartbeat loop and message polling
│   ├── ams-client.ts         # AMS API client (fleet, memory, execution)
│   ├── fleet-config.ts       # AMS fleet mode configuration
│   ├── task-scheduler.ts     # Cron, interval, one-shot tasks
│   ├── observatory-hooks.ts  # Real-time execution streaming
│   ├── mount-security.ts     # Host directory access control
│   ├── channels/
│   │   └── gateway.ts        # AMS Communications Gateway channel
│   └── ...
├── container/
│   ├── Dockerfile.abot       # Agent container image
│   ├── agent-runner/         # Code that runs inside each container
│   │   └── src/
│   │       ├── index.ts          # Agent execution loop
│   │       ├── ams-mcp-bridge.ts # SSE→stdio MCP tool bridge
│   │       └── ipc-mcp-stdio.ts  # Inter-process communication
│   └── skills/               # SKILL.md files synced to agents
├── docker-compose.yml
├── .env.example
└── package.json
```

## Configuration Reference

### Required

| Variable | Description |
|----------|-------------|
| `AMS_URL` | Your AMS instance URL |
| `AMS_AGENT_ID` | Agent ID (from AMS dashboard) |
| `AMS_TENANT_ID` | Tenant ID (from AMS dashboard) |
| `AMS_AGENT_TOKEN` | Authentication token |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `ASSISTANT_NAME` | `Abot` | Name used as trigger pattern in channels |
| `CONTAINER_IMAGE` | `automaton-agent:latest` | Docker image for agent containers |
| `MAX_CONCURRENT_CONTAINERS` | `5` | Maximum simultaneous agent containers |
| `IDLE_TIMEOUT` | `1800000` | Container idle timeout in ms (30 min) |
| `LOG_LEVEL` | `info` | Logging verbosity: debug, info, warn, error |
| `HEALTH_PORT` | `8080` | Health check endpoint port |
| `AMS_MCP_ENDPOINT` | derived from AMS_URL | MCP SSE endpoint for agent tools |
| `AMS_GATEWAY_URL` | derived from AMS_URL | Communications gateway URL |

### Container Networking (Docker-on-Docker)

| Variable | Default | Description |
|----------|---------|-------------|
| `AMS_DOCKER_NETWORK` | `ams_ams_network` | Docker network for agent containers |
| `AMS_CONTAINER_URL` | `http://ams-server:3001` | AMS URL as seen from inside containers |
| `AMS_CONTAINER_MCP_ENDPOINT` | `http://ams-mcp-sse:3002/sse` | MCP endpoint from inside containers |

## Development

```bash
npm run typecheck    # TypeScript type checking
npm test             # Run test suite
npm run format       # Prettier formatting
npm run build        # Compile TypeScript
```

## Docker Compose

For production deployments alongside AMS:

```bash
docker compose up -d
```

See `docker-compose.yml` for the full service definition.

## What is AMS?

The **Automaton Memory System** is the persistent memory infrastructure that powers Automaton Abots. It provides:

- **Hierarchical memory** — episodic, semantic, and procedural tiers with automatic consolidation
- **Hybrid search** — vector similarity + full-text keyword search with re-ranking
- **Knowledge graph** — memory linking with prerequisite resolution and graph traversal
- **Automaton engine** — executable skills with Bayesian learning that improve with every use
- **Fleet management** — multi-agent coordination with real-time Observatory dashboard
- **Communications gateway** — unified messaging across Telegram, Discord, Slack, WhatsApp

Learn more at [automaton-memory.com](https://automaton-memory.com)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT — see [LICENSE](LICENSE) for details.

---

<div align="center">

**Built by [Dead Reckoning Foundry](https://github.com/plundrpunk)**

*Agents that remember. Systems that learn. Infrastructure that lasts.*

</div>
