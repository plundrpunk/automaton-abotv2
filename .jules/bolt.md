## 2024-12-07 - Avoid heap allocations in network serialization structs
**Learning:** Rust `serde` payload structs often serialize dynamic data like timestamps or configurations without owning them. When those strings are generated on hot loops or in frequent intervals (like telemetry or execution logging), wrapping them as `&'a str` inside the payload object and letting the caller generate or own the String prevents redundant `.clone()` operations and reduces heap allocations.
**Action:** Use lifetimes like `<'a>` when creating temporary JSON serializable structures passed over the network for performance efficiency.
## 2024-12-07 - Avoid heap allocations when borrowing strings
**Learning:** In Rust, when passing optional strings (`Option<String>`) to functions or structs that only need read access, using `.clone()` causes expensive deep copies of potentially large heap-allocated strings, like system prompts. Using `.as_deref()` allows extracting an `Option<&str>` without any allocation overhead.
**Action:** When working with optional strings that only require read access on hot loops or execution paths, use `.as_deref()` instead of `.clone()` to avoid redundant heap allocations.
