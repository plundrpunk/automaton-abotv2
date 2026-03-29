# abot-v3

`abot-v3` is the current ABot runtime workspace: a Rust agent process that connects to AMS, matches a single seeded head by name, and executes automata inside a Wasmtime sandbox.

## What abot-v3 is today

- One ABot process runs one agent body.
- Repo-shipped bodies live under `hands/`.
- The 8 shipped bodies are seeded to match existing AMS heads by direct string name.
- The current runtime is still one-agent-per-process, so Docker Compose runs one container per body.

## Current matching contract

Today the safest matching contract for shipped bodies is direct name matching:

- `hands/<body-name>/` folder name
- AMS head name
- `AUTOMATON_AGENT_NAME`
- `AUTOMATON_AGENT_ID`

For shipped bodies, all four values should be the same string.

Example for `researcher`:

```text
hands/researcher == researcher == AUTOMATON_AGENT_NAME == AUTOMATON_AGENT_ID
```

## Docker outside, WASM inside

The deployment model is:

- Docker runs one ABot body per container for process and filesystem isolation.
- WASM/Wasmtime stays inside the body runtime as the sandbox for automata execution.

In other words: Docker is the outer isolation boundary for the body process, and WASM is the inner sandbox boundary for code the body executes.

## Run one body locally

From `abot-v3/`:

```bash
AUTOMATON_AGENT_NAME=researcher \
AUTOMATON_AGENT_ID=researcher \
cargo run -p abot-cli -- --config config/abot.toml
```

Equivalent helper script:

```bash
python3 scripts/run_hands.py run researcher
```

If AMS is not on `http://localhost:3001`, also set `AUTOMATON_AMS_URL`.

## Run one body in Docker

Build the image from `abot-v3/`:

```bash
docker build -t abot-v3 .
```

Run one body and match it to the seeded AMS head by name:

```bash
docker run --rm -it \
  --add-host=host.docker.internal:host-gateway \
  -e AUTOMATON_AMS_URL=http://host.docker.internal:3001 \
  -e AUTOMATON_AGENT_NAME=researcher \
  -e AUTOMATON_AGENT_ID=researcher \
  abot-v3
```

On macOS with Docker Desktop, `host.docker.internal` resolves automatically. The `--add-host` mapping is included so the same pattern also works on Linux Docker engines that support `host-gateway`.

## Run the built-in 8 in Docker Compose

From `abot-v3/`:

```bash
docker compose -f docker-compose.hands.yml up --build -d
```

This starts one container for each shipped body:

- `general-assistant`
- `researcher`
- `backend-engineer`
- `frontend-engineer`
- `technical-writer`
- `memory-curator`
- `data-analyst`
- `task-runner`

Each service uses the same image, the same `config/abot.toml`, and pins `AUTOMATON_AGENT_NAME` plus `AUTOMATON_AGENT_ID` to the service name.

## Add a custom body

1. Create a new folder under `hands/`, for example `hands/release-manager/`.
2. Add the body assets you want to ship there.
3. Create or seed the AMS head with the same name: `release-manager`.
4. Run the body with `AUTOMATON_AGENT_NAME=release-manager` and `AUTOMATON_AGENT_ID=release-manager`.
5. If using Compose, add another service that pins both env vars to `release-manager`.

The current contract is name based, so the body folder name, AMS head name, `AUTOMATON_AGENT_NAME`, and `AUTOMATON_AGENT_ID` should all match.

## Environment variables for Docker and Compose

See `abot-v3/.env.example` for the documented container-oriented variables.
