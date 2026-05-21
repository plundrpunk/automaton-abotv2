## 2024-04-30 - Serde Reference Optimation
**Learning:** Heartbeat payloads on an agent can become a bottleneck since they happen on an interval. Using owned types like `String` in `HeartbeatPayload` meant memory allocations via `.clone()` every tick.
**Action:** When serializing frequent metric/telemetry payloads with Serde, prefer reference lifetimes (`&'a str`) to avoid memory allocations and deep cloning of context strings.

## 2026-05-04 - LLM Request Payload Optimization
**Learning:** In the core LLM execution loop (`runtime.rs`), ownership of large message and tool arrays in `ToolCompletionRequest` forced full `.clone()` calls before each tool-completion request.
**Action:** Prefer borrowed request fields like `&[T]` and `&str` for serialization-only payload structs so hot loops can reuse existing data without heap cloning.

## 2024-05-15 - Serde Struct Lifetimes
**Learning:** When using lifetimes to avoid heap allocations on struct serialization, remember that Rust coercion to `&str` doesn't automatically occur inside `Option::Some()`. `Some(&String)` resolves to `Option<&String>`, not `Option<&str>`.
**Action:** Use `.as_str()` or `.as_deref()` explicitly on `String` values before wrapping them in `Some()` for a struct expecting `Option<&str>`.

## 2024-05-18 - Register Execution Payload Optimization
**Learning:** Registering executions happens frequently. Using owned types like String in RegisterExecutionRequest meant memory allocations via .clone() on every request.
**Action:** When serializing frequent metric/telemetry or execution payloads with Serde, prefer reference lifetimes (&'a str) to avoid memory allocations and deep cloning of context strings.

## 2026-05-18 - RuntimeState string allocation optimization
**Learning:** `RuntimeState` struct was holding owned `String` fields while being instantiated constantly in `tick()` loops. Because this object was passed directly to `HeartbeatReporter` and immediately mapped out and dropped, it resulted in a pointless `clone()` of strings per tick.
**Action:** Always prefer lifetimes and borrowed references for objects passed to synchronous or short-lived asynchronous boundaries where data ownership doesn't escape. Additionally, implemented an `as_str()` method on `AgentStatus` to prevent `to_string()` allocation on conversion.

## 2024-05-21 - Status Map String Allocation
**Learning:** `normalize_fleet_status` was called every heartbeat tick and used `to_ascii_lowercase()` and `.to_string()`, generating unnecessary allocations per heartbeat for simple string mapping.
**Action:** When mapping dynamic string inputs to a static vocabulary (e.g., telemetry status strings) in hot loops, avoid allocations by using `eq_ignore_ascii_case` and returning `&'static str` instead of allocating new `String`s.
