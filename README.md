# Automaton Abot

**Built by [DRF](https://github.com/plundrpunk) — the runtime that gives AI agents a body.**

Automaton Abot is an open-source Rust runtime for autonomous AI agents. Each Abot is a lightweight, sandboxed process that connects to [AMS](https://github.com/plundrpunk/agent-memory-backend) (Agent Memory System) for lifecycle management, persistent memory, and fleet coordination.

This is the **body**. AMS is the **head**.

> Fork the body, build your own agents, run them against any AMS instance.
> The body is MIT-licensed. Ship it, mod it, sell it.

---

## Architecture

```
┌─────────────────────────────────────────┐
│  AMS (proprietary head)                 │
│  Memory · Warden · Fleet · Trust Grants │
└──────────────────┬──────────────────────┘
                   │ HTTP/JSON
┌──────────────────▼──────────────────────┐
│  Abot Body (this repo, open source)     │
│  ┌───────────┐  ┌────────────────────┐  │
│  │ HAND.toml │  │ Rust Event Loop    │  │
│  │ (identity)│  │  birth → work →    │  │
│  │           │  │  heartbeat → death │  │
│  └───────────┘  └────────┬───────────┘  │
│                          │              │
│                 ┌────────▼───────────┐  │
│                 │ WASM Sandbox       │  │
│                 │ (automata execute  │  │
│                 │  here, isolated)   │  │
│                 └────────────────────┘  │
└─────────────────────────────────────────┘
```

- **Docker** isolates each body process
- **WASM** (Wasmtime) sandboxes the code each body runs
- **AMS** decides what the body is allowed to do (trust grants, not self-asserted)

## Trust Boundary

The body sends **claims** (who it is). AMS returns **grants** (what it can do).

A forked body cannot escalate its own privileges. Even if someone modifies the source to claim `trust_tier: 0` and `agent_class: admin`, AMS will look up the canonical grants from its database and return the real values. Unknown bodies get locked down: `trust_tier: 3`, `untrusted`, tools disabled, nanny-managed.

See [SECURITY.md](SECURITY.md) for the full threat model.

## Quick Start

### Prerequisites

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- A running AMS instance (default: `http://localhost:3001`)

### Build and run

```bash
cargo build --release

AUTOMATON_AGENT_NAME=researcher \
AUTOMATON_AGENT_ID=researcher \
AUTOMATON_AMS_URL=http://localhost:3001 \
./target/release/abot --config config/abot.toml
```

Or with the helper script:

```bash
python3 scripts/run_hands.py run researcher
```

### Run in Docker

```bash
docker build -t abot .

docker run --rm -it \
  --add-host=host.docker.internal:host-gateway \
  -e AUTOMATON_AMS_URL=http://host.docker.internal:3001 \
  -e AUTOMATON_AGENT_NAME=researcher \
  -e AUTOMATON_AGENT_ID=researcher \
  abot
```

### Run all 8 shipped bodies

```bash
docker compose -f docker-compose.hands.yml up --build -d
```

This spins up one container per body: `general-assistant`, `researcher`, `backend-engineer`, `frontend-engineer`, `technical-writer`, `memory-curator`, `data-analyst`, `task-runner`.

## Shipped Bodies

Each body lives in `hands/<name>/` with:

| File | Purpose |
|------|---------|
| `HAND.toml` | Identity manifest (name, archetype, domain, persona, goals) |
| `system_prompt.md` | LLM system prompt for this body's persona |
| `SKILL.md` | Skill documentation for tool use |

The matching contract is simple: the folder name = AMS head name = `AUTOMATON_AGENT_NAME` = `AUTOMATON_AGENT_ID`.

## Add Your Own Body

1. Create `hands/your-agent-name/`
2. Add `HAND.toml`, `system_prompt.md`, and optionally `SKILL.md`
3. Seed the matching head in your AMS instance
4. Run with `AUTOMATON_AGENT_NAME=your-agent-name`

No Rust required for persona customization — just edit the TOML and markdown.

## Workspace Crates

| Crate | What it does |
|-------|--------------|
| `abot-cli` | Binary entry point, CLI args, signal handling |
| `abot-core` | Event loop, HAND loader, config, runtime |
| `abot-ams` | AMS HTTP client, Warden protocol types |
| `abot-sandbox` | Wasmtime engine, fuel metering, permissions |
| `abot-security` | Ed25519 signing, Merkle audit, taint tracking |
| `abot-telemetry` | Heartbeat, Prometheus metrics |
| `abot-llm` | LLM provider routing (Kilo, direct) |
| `abot-mcp` | MCP client/server for tool use |
| `abot-channels` | Telegram, Discord, Slack adapters |

## Environment Variables

See [`.env.example`](.env.example) for the full list. The critical ones:

| Variable | Default | Purpose |
|----------|---------|---------|
| `AUTOMATON_AMS_URL` | `http://localhost:3001` | AMS server URL |
| `AUTOMATON_AMS_API_KEY` | (none) | API key for AMS auth |
| `AUTOMATON_AGENT_NAME` | from config | Body name (must match AMS head) |
| `AUTOMATON_AGENT_ID` | from config | Agent ID (usually same as name) |

## License

MIT — see [LICENSE](LICENSE).

Built by Drew Rutledge ([@plundrpunk](https://github.com/plundrpunk)). Part of the [Automaton](https://github.com/plundrpunk) ecosystem.
