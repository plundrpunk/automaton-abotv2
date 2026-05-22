1. **Edit `crates/abot-core/src/runtime.rs` to update the `ExecutionCompleteEvent` struct definition with a lifetime parameter.**
   - Change `ExecutionCompleteEvent` to `ExecutionCompleteEvent<'a>`.
   - Update `model` field to `Option<&'a str>` and `chunk_type` to `&'static str`.
2. **Edit `crates/abot-core/src/runtime.rs` to update the signatures of `run_single_shot` and `run_tool_loop`.**
   - Update return type of `run_single_shot` to `Result<ExecutionCompleteEvent<'a>>`. Add lifetime parameter `<'a>` to the function and apply it to `requested_model: &'a str`. Wait, what about `result.model` which is owned in `run_single_shot`? Actually, `GenerationResult` returns `model` as an owned `String`. So if we use `&'a str`, we won't be able to return `result.model.as_str()` because it would not outlive the function. The journal says:
     `When optimizing telemetry structs (e.g., ExecutionChunkRequest) with borrowed references (&str) across async boundaries, separate the owned data generation (e.g., returning an ExecutionCompleteEvent) from the short-lived serialization struct. This avoids complex lifetime issues while preventing heap allocations during serialization.`
     Wait, so `ExecutionCompleteEvent` *should* own its data, but it currently has `.to_string()` for `chunk_type` which is completely unnecessary! `chunk_type` can just be a `&'static str` without lifetime issues.
     Let's check the journal:
     "When mapping dynamic string inputs to a static vocabulary (e.g., telemetry status strings) in hot loops, avoid allocations by using `eq_ignore_ascii_case` and returning `&'static str` instead of allocating new `String`s with `to_ascii_lowercase()`."
     Wait! What does `ExecutionCompleteEvent` use?
     ```rust
     #[derive(Debug, Default)]
     struct ExecutionCompleteEvent {
         pub content: Option<String>,
         pub tokens_in: Option<u64>,
         pub tokens_out: Option<u64>,
         pub duration_ms: Option<u64>,
         pub error: Option<String>,
         pub model: Option<String>,
         pub chunk_type: String,
     }
     ```
     `chunk_type` is always instantiated with `"complete".to_string()` or `"error".to_string()`. This is a clear memory allocation that we can avoid. We just change `chunk_type` to be `&'static str` (and update default to `""`).

Let's revise the plan:
1. **Optimize `ExecutionCompleteEvent.chunk_type` to use `&'static str` instead of owned `String` in `crates/abot-core/src/runtime.rs`**
   - Change `chunk_type` type to `&'static str` in `ExecutionCompleteEvent`.
   - Remove `.to_string()` calls for `"complete"` and `"error"` when instantiating `ExecutionCompleteEvent` throughout `crates/abot-core/src/runtime.rs`.
   - In `process_message`, pass `completion_event.chunk_type` directly to `ExecutionChunkRequest` since it now expects a borrowed string and `ExecutionCompleteEvent` holds it.
2. **Run the full test suite** using `cargo test --workspace` to ensure no regressions were introduced.
3. **Run `cargo check -p abot-core`** to verify compilation.
4. **Complete pre commit steps** to ensure proper testing, verification, review, and reflection are done.
5. **Submit the PR** with a Bolt-themed title.

Wait, are there any other optimizations?
The `ExecutionCompleteEvent` has:
```rust
struct ExecutionCompleteEvent {
    pub content: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub model: Option<String>,
    pub chunk_type: &'static str,
}
```
If we look at `process_message`:
```rust
        self.ams
            .emit_execution_chunk(
                &fleet_execution_id,
                &ExecutionChunkRequest {
                    agent_id: &state.agent_id,
                    tenant_id: "default",
                    execution_id: &fleet_execution_id,
                    chunk_type: &completion_event.chunk_type,
                    timestamp: &timestamp,
                    data: ExecutionChunkData {
                        content: completion_event.content.as_deref(),
                        model: completion_event.model.as_deref(),
...
```
`chunk_type` is currently passed as `&completion_event.chunk_type`. If we change it to `&'static str`, it works.
Let's make sure this complies with the instructions.
