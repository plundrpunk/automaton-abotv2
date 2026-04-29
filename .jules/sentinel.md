## 2026-04-27 - [Argument Injection in std::process::Command]
**Vulnerability:** Argument injection via user-controlled positional arguments passed to `std::process::Command`.
**Learning:** Found in `crates/abot-llm/src/kilo.rs`, the user-provided `prompt` was passed directly to `std::process::Command` via `.arg(prompt)`. If the prompt began with a dash, the `kilo` binary could have parsed it as a flag instead of a positional argument.
**Prevention:** Always use the `--` separator before user-controlled positional arguments to signal the end of command-line options when using `std::process::Command`.
## 2026-04-28 - [String Indexing Panic in Rust]
**Vulnerability:** Panic due to byte-indexing on string slice when string contains multibyte UTF-8 characters.
**Learning:** Found in `crates/abot-security/src/manifest.rs`, the string slice `&sig_str[i * 2..i * 2 + 2]` was used directly. Since `.len()` returns length in bytes, a 128-byte string with multibyte characters could lead to panic since slicing on strings must be on character boundaries.
**Prevention:** Validate that the string is purely ASCII with `.is_ascii()` before performing byte-indexed string operations.
