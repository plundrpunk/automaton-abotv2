# PR #64 fleet rollout preflight — 2026-09-11

**Source preflight passes with the local build fixes below. Rollout readiness is
unconfirmed.** This audit did not connect to the VPS, build a release image, start
an Abot container, invoke providers, or deploy. The script only renders Compose
locally and inspects source. Host checks and live acceptance remain for a separately
authorized rollout session.

## Exact revisions and checkout

- [PR #64](https://github.com/plundrpunk/automaton-abotv2/pull/64),
  “Reconcile prod's unpushed hand-patches onto master”, merged at
  `2026-09-11T18:51:00Z` into `master`.
- Merge and freshly fetched `origin/master`:
  `006f8e981bb98cdd11256d15d9bcce761d994843`.
  PR head: `3f2bb3dbe771f461ba3abab6a4c8f6aaa727e350`.
  GitHub reports fmt, clippy, test and pr-merge-gate successful on the PR head;
  this is not a Docker build or production-health receipt.
- Requested checkout:
  `/Users/drfoundryos/Documents/DevFolder/automaton-abotv2`, branch
  `fix/mcp-server-start`, HEAD `c98c148873469cd4695e11a140e01dbfe37bdbb3`.
  It is **working-tree dirty** in `crates/abot-mcp/src/server.rs` (83 additions,
  13 deletions) and **base dirty**: 52 commits behind `origin/master`, zero ahead.
  Its Compose model has 22 services (8 legacy workers, Prime, 13 TLs).
  That checkout and its edit were preserved.
- Release source: the fixes below are **not** in `006f8e9`. They land on top of
  it in the build-pin PR that carries this document, and that merge commit —
  not `006f8e9` — is the first revision the fleet can actually be built from.

## Re-run the no-deploy source check

Requires installed Python 3.11+ and Docker Compose; no Python package is added.

```bash
cd <repo root>
python3 scripts/preflight_fleet.py
cargo fmt --all --check
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
git diff --check
```

The script ignores `.env` and inherited `COMPOSE_*` settings and uses an empty
API key with a fixed AMS URL. It checks the canonical model with both the default
`./ams` bind source and an explicit `/home/andrew/ams` source. It does not validate
deployment credentials, overlays, host paths or live services. Exit 0 means
source checks passed; the JSON deliberately retains `rollout_ready: false`.

## Compose and required identities

All 18 services have the same `Dockerfile`, build context, `abot-v3:local` image,
`abot --config config/abot.toml` command and `unless-stopped` restart policy.
They use the **existing external** Docker network `ams_ams_network`; Compose does
not create it. They publish no ports and have no profiles. AMS defaults to
`http://ams-server:3001`, with `AUTOMATON_AMS_API_KEY` supplied privately at rollout.
An empty rendered key proves only that parsing succeeds.

Every service mounts the host AMS checkout read-only at `/home/andrew/ams`.
The default host source is `./ams`, relative to the Compose project directory.
Set `AUTOMATON_HOST_AMS_DIR` explicitly to the verified host checkout; a missing
source must block rollout rather than leave a silently created empty directory.
There are no `/app/hands` overlays in the merged model: `hands/` and `config/`
are copied into the image, so prompt or hand changes require a new image.

| Compose service | Exact AMS name and ID / required seed name | Hand directory |
| --- | --- | --- |
| automaton-abot-prime-v2 | Automaton-Abot Prime V2 | hands/automaton-abot-prime-v2 |
| tl-academic | tl-academic | hands/tl-academic |
| tl-design | tl-design | hands/tl-design |
| tl-engineering | tl-engineering | hands/tl-engineering |
| tl-finance | tl-finance | hands/tl-finance |
| tl-gamedev | tl-gamedev | hands/tl-gamedev |
| tl-gis | tl-gis | hands/tl-gis |
| tl-healthcare | tl-healthcare | hands/tl-healthcare |
| tl-marketing | tl-marketing | hands/tl-marketing |
| tl-paid-media | tl-paid-media | hands/tl-paid-media |
| tl-product | tl-product | hands/tl-product |
| tl-project-mgmt | tl-project-mgmt | hands/tl-project-mgmt |
| tl-sales | tl-sales | hands/tl-sales |
| tl-security | tl-security | hands/tl-security |
| tl-spatial | tl-spatial | hands/tl-spatial |
| tl-specialized | tl-specialized | hands/tl-specialized |
| tl-support | tl-support | hands/tl-support |
| tl-testing | tl-testing | hands/tl-testing |

For each row, `AUTOMATON_AGENT_NAME`, `AUTOMATON_AGENT_ID`, `[hand].name`,
`[hand].agent_id` and `[matching].seed_name` match. Every manifest requires a seeded
head and has nonempty prompt and skill files. Prime additionally needs the explicit
`AUTOMATON_HAND_DIR=./hands/automaton-abot-prime-v2`; its displayed identity contains
spaces. All hands request the `codex` model through AMS routing.

Verify existing seeded heads and grants within the intended account/user scope in
the future host session; this audit did not query or seed them. The TL manifests
use `team-lead`; Prime uses `abot`. Prime therefore depends on granted tool enablement
or an explicit override, unlike the TL archetype path. The old snapshot sets
`AUTOMATON_ENABLE_TOOLS=true`; merged Compose does not. Check Prime's actual grant
and exposed tools before accepting orchestration. Do not infer server grants from
the local manifest's runtime hints or add an override during this audit.

`scripts/start_fleet.sh` launches the eight legacy host processes and is not the
18-service Compose launcher. Do not use it for this rollout.

## Concrete build defects and focused fixes

1. **Builder compiler too old.** Merged Dockerfile pins `rust:1.86-bookworm`,
   while `runtime.rs` and `heartbeat.rs` use Rust 2024 let chains, stabilized in
   [Rust 1.88](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/).
   The preflight rejects that original Dockerfile; the local pin is now
   `rust:1.88-bookworm`. README's obsolete 1.75 minimum is corrected.
2. **Build did not enforce the reviewed lockfile.** The release command now uses
   `cargo build --release --locked -p abot-cli`. No dependency versions or runtime
   behavior were changed. A lock mismatch now fails instead of resolving a new graph.
3. **Unfiltered source context.** The requested checkout has 1.5 GB in `target/`
   and no Docker ignore file. `Dockerfile.dockerignore` excludes Cargo output, Git
   metadata and environment files from the source build context. This file applies
   only to `Dockerfile`, preserving `Dockerfile.prebuilt`'s existing access to
   `target/release/abot`.

The source Dockerfile also installs Git and CA certificates in Debian Bookworm.
`Dockerfile.prebuilt` uses Ubuntu, does not install Git, and copies a preexisting
binary: it is not an equivalent fallback for this fleet. A Mac Mach-O binary
cannot serve as a Linux binary. Confirm the VPS architecture before selecting
the release image platform; the inspected local Docker engine is Linux **aarch64**.

The full Linux locked dependency graph is not cached on this Mac. Offline Linux
metadata stopped at missing `curve25519-dalek-derive v0.1.1`; an unfiltered metadata
request also lacked Android-only packages. Neither is a lockfile error. A Linux
release build needs the locked crates and Rust/Debian images available through an
approved registry path, plus apt access or a populated build cache. Local Rust
checks used the already installed Rust/Cargo 1.92 and cached macOS dependencies.
They do not certify a Linux release build under Rust 1.88.

## Disk and build prerequisites — future host checks

PR #64 reported 90% disk use, roughly 20 GB free and 12.8 GB reclaimable cache.
Those are historical PR observations, not a current measurement. Do not run its
prune suggestion automatically: pruning is destructive and may remove rollback
material. Nothing was pruned in this audit.

Local observations: about 130 GiB free before validation and 127.7 GiB in the source
receipt; Docker 29.4.0, Compose 5.1.2, Buildx 0.33.0 on the verified Unix-socket
OrbStack context. Local Docker reported 21.39 GB images, 7.956 GB volumes and
24.5 GB build cache. These describe the Mac, not the VPS.

Before authorizing a host build, record:

- Fresh filesystem space and inodes for the source checkout, Docker data root,
  build temporary storage and rollback backup destination (`df -h`, `df -i`,
  `docker info --format '{{.DockerRootDir}}'`, `docker system df`).
- Available RAM/swap and build concurrency. There is one shared image; build it
  once via one named Compose service after approval, then verify all 18 consume
  that exact image ID. Avoid budgeting for 18 independent release compilations.
- Space for old image retention, rollback export if needed, build layers, Cargo
  release intermediates, final image, logs and an operating reserve. No measured
  clean Linux peak exists in this audit, so neither historical 20 GB free nor the
  Mac's free space earns a host pass. Set a capacity budget before building and
  stop if measured headroom cannot cover it.
- Exact clean release revision (including the reviewed fixes), actual Compose
  project name, effective Compose files/overrides, architecture and source paths.
  Preserve the running project's name; a different name can create a duplicate
  fleet with duplicate AMS identities.
- Presence of `ams_ams_network`, reachable `ams-server:3001` from that network,
  a real readable host AMS checkout and privately supplied credentials. Use
  `compose config --quiet`/`--services` for operator checks; raw rendered configs
  and `docker inspect .Config.Env` can reveal secrets and should not enter receipts.

This repository has no `deploy-check.sh`. Do not invent a successful deploy gate;
the available source checks and the remaining host/live checks are listed here.

## Health acceptance — required after separately authorized startup

**There is no Docker `HEALTHCHECK`, Compose healthcheck, health endpoint in the
Abot CLI, or exposed metrics listener to use as a readiness probe.** The configured
metrics port is not proof of a listener. `up --wait`, a running PID, or an AMS
`/health` response alone cannot establish that all identities are healthy.

For a future rollout, capture a baseline before replacement: the exact 18 service
names, container IDs, image IDs, restart counts, current execution IDs and Warden/
fleet records. First establish an idle/drained window; do not restart active work
or replay in-flight provider requests as part of a health check.

Suggested acceptance for an **idle** fleet (an operator gate, not an implemented
SLO): allow up to 90 seconds for initial registration, then observe at least three
advancing samples at the configured 10-second heartbeat interval, with each
identity's latest heartbeat no older than 30 seconds and no restart-count growth.

| Check | Required evidence |
| --- | --- |
| Process/image | Exactly the expected service set in the existing project, one running container per service, approved image ID and no restart loop |
| Identity/assets | Correct exact IDs, manifest loaded, prompt present; no missing-hand warnings or unintended hand mounts |
| Birth/grants | `Birth ritual complete`, expected `AMS grants received`; no unrecognized-head or missing-grant warning |
| Both heartbeat channels | Warden **and** fleet records advance for all 18 IDs and match the new container instances; do not count stale rows |
| Registration | `Fleet registration complete`, or demonstrated successful subsequent fleet upsert; failures otherwise remain unresolved |
| Logs | No recurring `Heartbeat failed`, `Fleet heartbeat failed`, poll failures, auth errors, missing model routes or steering errors |
| Functional acceptance | With separate permission for live execution, a bounded TL journey exposes `poll_worker_execution`, dispatches once, preserves correlation/execution ID, and fans in the terminal result; Prime's required tools also work |

`list_fleet_agents` and `warden_status` are the PR's proposed observability tools;
use their intended account scope and inspect per-identity freshness. Do not accept
an aggregate count of 18. `HeartbeatReporter::tick` sends both channels, but fleet
failures are logged and ignored; the event loop also retries Warden failures while
the process stays alive. This is why process status is insufficient. Work execution
can occupy the event loop, so the idle heartbeat window must not be treated as a
validated guarantee during long tasks.

No live functional smoke was run here. Passing local dispatch/timeout tests does
not prove live gateway, provider, memory-write or fan-in behavior.

## Rollback target and evidence still required

**Source rollback:** `cca8e94e9d87528e7477ef2b7e152a7926ddccfa`, the production
snapshot identified by PR #64. Locally it resolves at
`refs/prodsnap/prod/snapshot-20260911`. Its Compose model renders to 18 services,
and all 18 snapshot hand IDs match. This is not the merge's first parent
`37d4235e707066cb9118408e8d7dbffd50b03b3d`.

**Reported VPS recovery copies, not inspected here:** branch
`prod/snapshot-20260911` and
`~/backups/abotv2-prod-snapshot-20260911.bundle`. The future operator must verify
the bundle with `git bundle verify` and confirm the contained ref resolves to the
full snapshot SHA. Source availability alone does not preserve the runnable image.

Before any rebuild overwrites mutable `abot-v3:local`, record the running image ID
for **each** service. If IDs differ, preserve the complete per-service mapping.
Protect those exact images under unique rollback tags and, where necessary, a
verified image export, keeping their disk needs in the budget. No rollback image
ID or export was available to this audit, so binary rollback readiness is unconfirmed.

Also preserve the effective Compose/environment configuration privately, project
name and required mounted assets. The snapshot uses 17 host hand-directory
overlays and sets `AUTOMATON_ENABLE_TOOLS=true`; restoring only an old image under
the new Compose file does not reconstruct that snapshot. Its old Dockerfile also
pins Rust 1.86, so rebuilding the snapshot is not a dependable emergency path.

Future rollback procedure, only after authorization: restore the recorded source
and effective Compose/assets without discarding new uncommitted work; select the
preserved image IDs via an explicit image override; recreate the same project
without build/pull; repeat the same per-identity health acceptance. Verify rollback
coverage before approving rollout. Do not use `down -v`, image/cache pruning, or a
new build as the rollback strategy.

## Validation receipts

| Check | Result and proof boundary |
| --- | --- |
| Merged PR and local state | PASS — GitHub PR metadata and fresh fetch; checkout preserved |
| Canonical Compose, both mount variants, 18 identities/assets | PASS — [source-check.json](preflight-20260911/source-check.json) |
| Seven negative cases, including original Rust 1.86 Dockerfile | PASS — [negative-checks.json](preflight-20260911/negative-checks.json); the original pin is deliberately rejected |
| Rust tests | PASS — 82 tests, 0 failures; [cargo-test.log](preflight-20260911/cargo-test.log) |
| Formatting and diff whitespace | PASS — `cargo fmt --all --check`, `git diff --check` |
| Strict Clippy | PASS — [clippy.log](preflight-20260911/clippy.log) |
| Docker static build check | PASS — [docker-build-check.log](preflight-20260911/docker-build-check.log); `buildx build --check`, no warnings, base-tag metadata resolved |
| Snapshot source and Compose identities | PASS — local full snapshot SHA, 18 matching identities |
| Linux release image build/architecture/runtime | NOT TESTED — static Docker check does not compile or run an image |
| VPS disk/network/mounts/auth/seeds/grants | NOT TESTED — no VPS connection |
| Startup/heartbeats/live fan-in/rollback rehearsal | NOT TESTED — no fleet startup or live calls |
| Browser/mobile/oversized UI input | NOT APPLICABLE — Rust daemon/CLI, no UI in this task |

The first restricted Rust run failed only when its localhost mock listener was
denied with `Operation not permitted`. Rerunning offline with local socket access
passed all 82 tests. The source receipt describes the merge plus these local fixes;
it is not an immutable release attestation. Review the focused diff, record its
commit, and perform the remaining host preflight in a separately authorized session.
