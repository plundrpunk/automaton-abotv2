## 2026-04-27 - [Argument Injection in std::process::Command]
**Vulnerability:** Argument injection via user-controlled positional arguments passed to `std::process::Command`.
**Learning:** Found in `crates/abot-llm/src/kilo.rs`, the user-provided `prompt` was passed directly to `std::process::Command` via `.arg(prompt)`. If the prompt began with a dash, the `kilo` binary could have parsed it as a flag instead of a positional argument.
**Prevention:** Always use the `--` separator before user-controlled positional arguments to signal the end of command-line options when using `std::process::Command`.

## 2026-04-28 - [String Slicing Panic via Multi-byte Characters]
**Vulnerability:** Denial of Service (DoS) due to panics caused by byte-indexing strings containing multi-byte UTF-8 characters without checking char boundaries.
**Learning:** Found in `crates/abot-security/src/manifest.rs` in `decode_signature`. String slices in Rust must align with character boundaries; otherwise, the program panics. The hex signature parser used `&sig_str[i*2..i*2+2]` assuming a pure hex string, but an attacker could supply a 128-byte string containing multi-byte characters to trigger the panic.
**Prevention:** Always validate strings are pure ASCII using `.is_ascii()` before performing raw byte-indexed slicing on them, or iterate over characters/bytes safely.

## 2026-05-18 - [Secret Leakage via Derived Debug Implementations]
**Vulnerability:** Exposure of sensitive configuration values (like API keys and tokens) in logs and debugging output.
**Learning:** Found in `crates/abot-ams/src/client.rs` (`AmsConfig`) and `crates/abot-core/src/config.rs` (`ChannelAdapterConfig`). By default, deriving `Debug` will output all fields of a struct. If a struct containing secrets is printed or logged using `{:?}`, those secrets are leaked in plaintext.
**Prevention:** Do not use `#[derive(Debug)]` on structs containing sensitive information. Instead, manually implement `std::fmt::Debug` and explicitly redact the sensitive fields by formatting them as `***` or similar.

## 2026-05-24 - [Path Traversal in Sandbox Allowed Paths Check]
**Vulnerability:** Path traversal vulnerability escaping sandbox limitations via unresolved `..` components.
**Learning:** Found in `crates/abot-sandbox/src/permissions.rs`. The `Path::starts_with` method in Rust performs a simple lexical component comparison and does not resolve `.` or `..` components in the path. An attacker could provide a path like `/tmp/sandbox/../etc/passwd`, which technically "starts with" the `/tmp/sandbox` prefix according to `starts_with`, thereby bypassing the sandbox directory constraints.
**Prevention:** Always logically normalize paths by resolving `.` and `..` components before performing prefix-based path restriction checks (like `starts_with`) to securely evaluate directory traversal boundaries.
