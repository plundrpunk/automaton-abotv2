## 2026-04-27 - [Argument Injection in std::process::Command]
**Vulnerability:** Argument injection via user-controlled positional arguments passed to `std::process::Command`.
**Learning:** Found in `crates/abot-llm/src/kilo.rs`, the user-provided `prompt` was passed directly to `std::process::Command` via `.arg(prompt)`. If the prompt began with a dash, the `kilo` binary could have parsed it as a flag instead of a positional argument.
**Prevention:** Always use the `--` separator before user-controlled positional arguments to signal the end of command-line options when using `std::process::Command`.
