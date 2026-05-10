## 2024-04-30 - Serde Reference Optimation
**Learning:** Heartbeat payloads on an agent can become a bottleneck since they happen on an interval. Using owned types like `String` in `HeartbeatPayload` meant memory allocations via `.clone()` every tick.
**Action:** When serializing frequent metric/telemetry payloads with Serde, prefer reference lifetimes (`&'a str`) to avoid memory allocations and deep cloning of context strings.

## 2026-05-04 - LLM Request Payload Optimization
**Learning:** In the core LLM execution loop (`runtime.rs`), ownership of large message and tool arrays in `ToolCompletionRequest` forced full `.clone()` calls before each tool-completion request.
**Action:** Prefer borrowed request fields like `&[T]` and `&str` for serialization-only payload structs so hot loops can reuse existing data without heap cloning.
## 2024-05-10 - Fleet Heartbeat Serialization Optimization
**Learning:** `build_fleet_payload` is called on every telemetry tick, and it was cloning three internal runtime `String` fields (agent ID, tenant ID, container ID) and allocating a new `String` for the status. Frequent telemetry payloads should avoid allocations.
**Action:** When creating high-frequency Serde payload structs like `FleetHeartbeatRequest`, use lifetimes `&'a str` instead of `String` for string values where the underlying data outlives the struct's serialization scope.
