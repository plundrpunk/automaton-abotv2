## 2024-04-30 - Serde Reference Optimation
**Learning:** Heartbeat payloads on an agent can become a bottleneck since they happen on an interval. Using owned types like `String` in `HeartbeatPayload` meant memory allocations via `.clone()` every tick.
**Action:** When serializing frequent metric/telemetry payloads with Serde, prefer reference lifetimes (`&'a str`) to avoid memory allocations and deep cloning of context strings.
