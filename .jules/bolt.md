## 2024-04-30 - Serde Reference Optimation
**Learning:** Heartbeat payloads on an agent can become a bottleneck since they happen on an interval. Using owned types like `String` in `HeartbeatPayload` meant memory allocations via `.clone()` every tick.
**Action:** When serializing frequent metric/telemetry payloads with Serde, prefer reference lifetimes (`&'a str`) to avoid memory allocations and deep cloning of context strings.

## 2026-05-04 - LLM Request Payload Optimization
**Learning:** In the core LLM execution loop (`runtime.rs`), ownership of large message and tool arrays in `ToolCompletionRequest` forced full `.clone()` calls before each tool-completion request.
**Action:** Prefer borrowed request fields like `&[T]` and `&str` for serialization-only payload structs so hot loops can reuse existing data without heap cloning.
