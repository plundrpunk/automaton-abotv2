## 2024-04-30 - Serde Reference Optimation
**Learning:** Heartbeat payloads on an agent can become a bottleneck since they happen on an interval. Using owned types like `String` in `HeartbeatPayload` meant memory allocations via `.clone()` every tick.
**Action:** When serializing frequent metric/telemetry payloads with Serde, prefer reference lifetimes (`&'a str`) to avoid memory allocations and deep cloning of context strings.

## 2026-05-04 - LLM Request Payload Optimization
**Learning:** In the core LLM execution loop (`runtime.rs`), ownership of large message and tool arrays in `ToolCompletionRequest` forced full `.clone()` calls before each tool-completion request.
**Action:** Prefer borrowed request fields like `&[T]` and `&str` for serialization-only payload structs so hot loops can reuse existing data without heap cloning.

## 2024-05-09 - Telemetry Heartbeat Micro-Optimization Rejection
**Learning:** Optimizing interval-based telemetry payloads (like `FleetHeartbeatRequest`) by switching from owned `String` fields to borrowed `&str` lifetimes can be viewed as unnecessary noise with "no real value add", particularly when the payload structure is simple and garbage collection/allocator pressure isn't a proven bottleneck.
**Action:** Avoid micro-optimizing payload serialization lifetimes unless there is a proven bottleneck, especially if it requires cluttering the code with lifetime annotations (e.g., `<'a>`). Prioritize code readability over saving a few string clones on periodic intervals.
