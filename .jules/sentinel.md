## 2024-05-24 - [CRITICAL] Prevent Argument/Flag Injection in `std::process::Command`
**Vulnerability:** Argument injection (flag injection) in `kilo` executable invocation via `Command::new()`. The `prompt` string from a user or LLM was passed directly after `mode.as_flag()`. If the prompt started with a `-`, the binary might try to parse it as an argument/flag.
**Learning:** `std::process::Command` does not use a shell (unlike `sh -c`), so shell injection (e.g. `; rm -rf /`) isn't possible, but *argument injection* is still a major risk if positional parameters can be misinterpreted as options by the target executable.
**Prevention:** Always use the `--` standard option terminator before passing variable or user-controlled positional arguments.
