# Shipped ABot v3 Bodies

`abot-v3/hands/` contains repo-shipped body assets for the 8 default AMS agents seeded from `seeds/base-agents.json`:

- `general-assistant`
- `researcher`
- `backend-engineer`
- `frontend-engineer`
- `technical-writer`
- `memory-curator`
- `data-analyst`
- `task-runner`

Each body directory contains:

- `HAND.toml` - future-friendly hand manifest with identity, persona, runtime hints, tool permissions, and goals
- `SKILL.md` - concise role card for the shipped body
- `system_prompt.md` - runtime-facing prompt text for that body

## Matching Assumption

Current AMS head matching is string-based in practice. The safest shipped convention is:

- `agent_name == seeded agent name`
- `agent_id == seeded agent name`

The launcher in `abot-v3/scripts/run_hands.py` pins both `AUTOMATON_AGENT_NAME` and `AUTOMATON_AGENT_ID` to the selected body name so an ABot process auto-matches the existing seeded AMS head without requiring UUID-first wiring.

This same direct name-matching rule is also the Docker rule documented in `abot-v3/README.md`: one container runs one body, and the container sets both `AUTOMATON_AGENT_NAME` and `AUTOMATON_AGENT_ID` to the body folder name.

## Launching

List the shipped bodies:

```bash
python3 abot-v3/scripts/run_hands.py list
```

Run one body:

```bash
python3 abot-v3/scripts/run_hands.py run researcher
```

Run all shipped bodies:

```bash
python3 abot-v3/scripts/run_hands.py run-all
```

Run the shipped bodies in Docker Compose instead:

```bash
docker compose -f abot-v3/docker-compose.hands.yml up --build -d
```

Useful flags:

- `--config /path/to/abot.toml` to override the default `abot-v3/config/abot.toml`
- `--log-level debug` to change runtime verbosity
- `--binary /path/to/abot` to force a specific compiled binary
- `--cargo` to force `cargo run -p abot-cli --` even if a compiled binary exists
- extra runtime args can be appended after `--`

## Runtime Scope

These shipped bodies are assets plus a launcher mechanism. The Rust runtime still runs one agent per process, and the `hands` config block is not yet wired into an in-process loader.

For deployment, that means Docker is the outer isolation layer for each body process, while WASM/Wasmtime remains the inner execution sandbox for automata that run inside that body. Docker and WASM are complementary here, not competing models.
