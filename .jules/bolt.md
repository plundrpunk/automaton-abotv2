## 2024-12-07 - Avoid heap allocations in network serialization structs
**Learning:** Rust `serde` payload structs often serialize dynamic data like timestamps or configurations without owning them. When those strings are generated on hot loops or in frequent intervals (like telemetry or execution logging), wrapping them as `&'a str` inside the payload object and letting the caller generate or own the String prevents redundant `.clone()` operations and reduces heap allocations.
**Action:** Use lifetimes like `<'a>` when creating temporary JSON serializable structures passed over the network for performance efficiency.

## 2024-12-08 - Use `as_deref()` instead of `clone()` for `Option<String>` in hot paths
**Learning:** In Rust, deeply cloning `Option<String>` when only a reference `Option<&str>` is required incurs unnecessary heap allocations. This is especially problematic for large strings, like LLM system prompts, which are passed continuously on the hot path (execution loops).
**Action:** Use `.as_deref()` instead of `.clone()` when mapping `Option<String>` to `Option<&str>` for read-only access.

## 2024-12-09 - Avoid breaking API changes for micro-optimizations
**Learning:** While returning `std::borrow::Cow<'_, str>` instead of `String` from a method is a common pattern to avoid allocations, doing so on a public API (`pub fn content_text()`) constitutes a breaking change and should be avoided in favor of refactoring internal variable usage.
**Action:** Use `.as_deref()` and similar techniques to extract `&str` from `Option<String>` safely without changing public struct/function signatures, avoiding breaking existing callers.
