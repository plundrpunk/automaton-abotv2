## 2024-12-07 - Avoid heap allocations in network serialization structs
**Learning:** Rust `serde` payload structs often serialize dynamic data like timestamps or configurations without owning them. When those strings are generated on hot loops or in frequent intervals (like telemetry or execution logging), wrapping them as `&'a str` inside the payload object and letting the caller generate or own the String prevents redundant `.clone()` operations and reduces heap allocations.
**Action:** Use lifetimes like `<'a>` when creating temporary JSON serializable structures passed over the network for performance efficiency.

## 2024-12-08 - Use `as_deref()` instead of `clone()` for `Option<String>` in hot paths
**Learning:** In Rust, deeply cloning `Option<String>` when only a reference `Option<&str>` is required incurs unnecessary heap allocations. This is especially problematic for large strings, like LLM system prompts, which are passed continuously on the hot path (execution loops).
**Action:** Use `.as_deref()` instead of `.clone()` when mapping `Option<String>` to `Option<&str>` for read-only access.
## 2024-12-09 - Use `as_deref()` with `Option<String>` for serialization and serialization mapping
**Learning:** We replaced `memory_id.clone().unwrap_or_default()` with `memory_id.as_deref().unwrap_or_default()` where `memory_id` was of type `Option<String>`. Calling `clone()` creates an entirely new heap allocation of the String, while `as_deref()` converts `Option<String>` into `Option<&str>` without any heap allocation, saving significant time when processing large strings or in tight loops.
**Action:** When dealing with `Option<String>` where we just want a default empty slice or string reference (`""`), we should use `.as_deref().unwrap_or_default()` rather than `.clone().unwrap_or_default()`.
