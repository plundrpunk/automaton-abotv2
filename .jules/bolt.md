## 2024-12-07 - Avoid heap allocations in network serialization structs
**Learning:** Rust `serde` payload structs often serialize dynamic data like timestamps or configurations without owning them. When those strings are generated on hot loops or in frequent intervals (like telemetry or execution logging), wrapping them as `&'a str` inside the payload object and letting the caller generate or own the String prevents redundant `.clone()` operations and reduces heap allocations.
**Action:** Use lifetimes like `<'a>` when creating temporary JSON serializable structures passed over the network for performance efficiency.

## 2024-12-08 - Use `as_deref()` instead of `clone()` for `Option<String>` in hot paths
**Learning:** In Rust, deeply cloning `Option<String>` when only a reference `Option<&str>` is required incurs unnecessary heap allocations. This is especially problematic for large strings, like LLM system prompts, which are passed continuously on the hot path (execution loops).
**Action:** Use `.as_deref()` instead of `.clone()` when mapping `Option<String>` to `Option<&str>` for read-only access.

## 2024-12-09 - Use Cow to prevent allocation from json strings
**Learning:** When extracting text out of a `serde_json::Value` element (e.g. `SteeringMessage::content`), you might face an issue where `Value::String` requires a `.clone()` or `.to_string()` to return an owned string, adding an overhead heap allocation.
**Action:** Use `std::borrow::Cow<'_, str>` to conditionally yield an owned string via `.to_string()` for generic objects while using `Cow::Borrowed` to yield borrowed references directly from the internal string value to avoid deep copying of string structures.
