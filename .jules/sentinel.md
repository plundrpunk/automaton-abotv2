## 2026-04-27 - [Argument Injection in std::process::Command]
**Vulnerability:** Argument injection via user-controlled positional arguments passed to `std::process::Command`.
**Learning:** Found in `crates/abot-llm/src/kilo.rs`, the user-provided `prompt` was passed directly to `std::process::Command` via `.arg(prompt)`. If the prompt began with a dash, the `kilo` binary could have parsed it as a flag instead of a positional argument.
**Prevention:** Always use the `--` separator before user-controlled positional arguments to signal the end of command-line options when using `std::process::Command`.
## 2024-04-30 - [DoS via Rust String Slicing Panic]
**Vulnerability:** Byte-indexed string slicing on unvalidated strings can cause panics and DoS.
**Learning:** Rust `&str[start..end]` uses byte indices, not character indices. If an attacker provides multi-byte characters (e.g. emojis) and the code slices it at a non-character boundary, Rust will panic and crash the program.
**Prevention:** Always validate that strings are ASCII (`is_ascii()`) before doing byte-indexed slicing, or use character iterators (`.chars()`) instead.
