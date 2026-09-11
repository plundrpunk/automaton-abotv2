# Tool Gap Fix Notes

Date: 2026-08-12

## What changed

- Added an AMS client task-board read call:
  - `crates/abot-ams/src/client.rs:397` adds `AmsClient::get_task_board()`.
  - It uses `GET /api/v1/goals/dashboard` through the existing `request()` helper, so it uses the same base URL and `X-API-Key` auth behavior as the rest of the AMS client.

- Added runtime task-board summarization:
  - `crates/abot-core/src/runtime.rs:99` adds status normalization for pending/active/done/failed.
  - `crates/abot-core/src/runtime.rs:127` collects task titles from the AMS dashboard payload.
  - `crates/abot-core/src/runtime.rs:159` returns the compact summary shape used by the tool.

- Added `create_memory` tool execution:
  - `crates/abot-core/src/runtime.rs:1320` handles `create_memory`.
  - Params: `content` required, `tier` defaulting to `episodic`, `tags` array of strings, optional `title`.
  - It writes through the existing `self.ams.create_memory(...)` path, which posts to `/api/v1/memories/` in `crates/abot-ams/src/client.rs:185`.

- Added `get_task_board` tool execution:
  - `crates/abot-core/src/runtime.rs:1406` handles `get_task_board`.
  - It returns `{ counts: { pending, active, done, failed }, top_tasks: [...] }`.

- Added tool schema entries so models can see both tools:
  - `crates/abot-core/src/runtime.rs:1493` defines the shared `create_memory` schema.
  - `crates/abot-core/src/runtime.rs:1531` defines the shared `get_task_board` schema.
  - TL tool list includes them at `crates/abot-core/src/runtime.rs:1643`.
  - Orchestrator tool list includes them at `crates/abot-core/src/runtime.rs:1748`.
  - MCP-bridge tool list includes them at `crates/abot-core/src/runtime.rs:1823`.

## Verification

- Ran `cargo check` successfully on the host.
- Existing warnings remain in unrelated crates (`abot-sandbox`, `abot-mcp`, `abot-channels`, `abot-telemetry`).

## Rebuild

```bash
docker build -t abot-v3:local .
docker compose -f docker-compose.hands.yml up -d --force-recreate
```

No image rebuild, container restart, or git commit was performed.
