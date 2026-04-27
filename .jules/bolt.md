## 2026-04-27 - String Truncation in Rust
**Learning:** Slicing strings by bytes (e.g. `&s[..100]`) can panic if the index lands on a multi-byte character. However, iterating characters (`chars().count()` and `chars().take().collect()`) is an O(n) operation and significantly slower. A middle ground is using byte slicing while adjusting the index to the nearest character boundary using `is_char_boundary(index)`.
**Action:** Use byte slicing with `is_char_boundary()` fallback instead of char iterators for high-performance string truncation.
