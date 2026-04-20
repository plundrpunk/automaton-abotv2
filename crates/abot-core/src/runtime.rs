use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::config::AbotConfig;
use crate::hand::{LoadedHand, load_hand};
use abot_ams::client::{AmsClient, AmsConfig, SteeringMessage};
use abot_ams::fleet::{ExecutionChunkData, ExecutionChunkRequest, RegisterExecutionRequest};
use abot_ams::llm::{CompletionRequest, ToolCompletionRequest};
use abot_ams::warden::{BirthRequest, Directive};
use abot_telemetry::heartbeat::{HeartbeatReporter, RuntimeState as TelemetryState};
use abot_llm::KiloBridge;
use abot_llm::kilo::KiloMode;

/// The main runtime event loop for the Abot.
///
/// This is the "dumb body" — it executes tasks, counts tokens,
/// and reports telemetry. All lifecycle decisions come from AMS.
pub struct Runtime {
    config: AbotConfig,
    ams: AmsClient,
    heartbeat: HeartbeatReporter,
    shutdown_rx: mpsc::Receiver<()>,
    hand: Option<LoadedHand>,
}

/// Current runtime state reported to AMS via heartbeat.
pub struct RuntimeState {
    pub agent_id: String,
    pub status: AgentStatus,
    pub context_pct: f64,
    pub current_execution: Option<String>,
    pub token_count: u64,
    pub max_tokens: u64,
}

struct GenerationResult {
    content: String,
    model: String,
    provider: String,
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum AgentStatus {
    Booting,
    Idle,
    Working,
    Dying,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Booting => write!(f, "booting"),
            Self::Idle => write!(f, "idle"),
            Self::Working => write!(f, "working"),
            Self::Dying => write!(f, "dying"),
        }
    }
}

impl Runtime {
    pub fn new(config: AbotConfig, shutdown_rx: mpsc::Receiver<()>) -> Result<Self> {
        // Convert core's config::AmsConfig → abot_ams::AmsConfig
        let ams_config = AmsConfig {
            url: config.ams.url.clone(),
            api_key: config.ams.api_key.clone(),
            connect_timeout_ms: config.ams.connect_timeout_ms,
            request_timeout_ms: config.ams.request_timeout_ms,
            heartbeat_interval_secs: config.ams.heartbeat_interval_secs,
        };
        let ams = AmsClient::new(&ams_config)?;
        let heartbeat = HeartbeatReporter::new(
            ams.clone(),
            config.ams.heartbeat_interval_secs,
        );

        // Load hand manifest if a matching hands/<agent_name>/ directory exists
        let hand = load_hand(&config.hands.directory, &config.agent.name);

        Ok(Self {
            config,
            ams,
            heartbeat,
            shutdown_rx,
            hand,
        })
    }

    /// Main entry point. Performs birth ritual, enters event loop, handles death.
    pub async fn run(&mut self) -> Result<()> {
        info!(agent = %self.config.agent.name, "Abot v3 starting");

        // === BIRTH RITUAL ===
        // Build birth claims: identity + runtime info only.
        // SECURITY: We send claims (who we are), not grants (what we're
        // allowed to do). AMS is the authority on trust_tier, agent_class,
        // tool_permissions, and thresholds — the body cannot self-escalate.
        let mut birth_metadata = serde_json::json!({
            "version": "3.0.0",
            "runtime": "rust",
            "sandbox": self.config.sandbox.engine,
        });
        if let Some(hand) = &self.hand {
            let claims = hand.to_ams_claims();
            if let (Some(base), Some(extra)) = (birth_metadata.as_object_mut(), claims.as_object()) {
                for (k, v) in extra {
                    base.insert(k.clone(), v.clone());
                }
            }
            if hand.system_prompt.is_some() {
                birth_metadata["has_system_prompt"] = serde_json::json!(true);
            }
        }

        let birth_response = self.ams.birth(BirthRequest {
            agent_id: self.config.agent.id.clone(),
            agent_name: self.config.agent.name.clone(),
            metadata: birth_metadata,
        }).await?;

        // Log AMS-granted operating limits
        if let Some(grants) = &birth_response.grants {
            info!(
                agent_id = %self.config.agent.id,
                trust_tier = grants.trust_tier,
                agent_class = %grants.agent_class,
                enable_tools = grants.enable_tools,
                max_iterations = grants.max_iterations,
                warn_threshold = grants.warn_threshold,
                critical_threshold = grants.critical_threshold,
                "AMS grants received — operating limits set by server"
            );
        } else {
            warn!(
                agent_id = %self.config.agent.id,
                "No AMS grants received — body may be unrecognized"
            );
        }

        info!(
            agent_id = %self.config.agent.id,
            continuation = ?birth_response.continuation,
            "Birth ritual complete"
        );

        // If we got a continuation, load it as our initial task context
        let mut state = RuntimeState {
            agent_id: self.config.agent.id.clone(),
            status: AgentStatus::Idle,
            context_pct: 0.0,
            current_execution: None,
            token_count: 0,
            max_tokens: 200_000, // Default, overridden by AMS config
        };

        if let Some(continuation) = &birth_response.continuation {
            info!(
                continuation_id = %continuation.continuation_id,
                goal = %continuation.original_goal,
                "Resuming from continuation"
            );
            // TODO: Load priority memories, set initial task context
        }

        // === MAIN EVENT LOOP ===
        let mut heartbeat_interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.ams.heartbeat_interval_secs)
        );
        let mut message_poll_interval = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            // Convert core RuntimeState → telemetry RuntimeState for heartbeat
            let telemetry_state = TelemetryState {
                agent_id: state.agent_id.clone(),
                context_pct: state.context_pct,
                status: state.status.to_string(),
                current_execution: state.current_execution.clone(),
            };

            tokio::select! {
                // Heartbeat tick — report to AMS, receive directive
                _ = heartbeat_interval.tick() => {
                    match self.heartbeat.tick(&telemetry_state).await {
                        Ok(directive) => {
                            match directive {
                                Directive::Continue => {},
                                Directive::Warn => {
                                    warn!(
                                        context_pct = state.context_pct,
                                        "AMS warns: approaching context limit"
                                    );
                                },
                                Directive::BeginDeathRitual => {
                                    info!("AMS directive: begin death ritual");
                                    state.status = AgentStatus::Dying;
                                    self.execute_death_ritual(&state).await?;
                                    return Ok(());
                                },
                                Directive::HardStop => {
                                    error!("AMS directive: HARD STOP");
                                    state.status = AgentStatus::Dying;
                                    self.execute_death_ritual(&state).await?;
                                    return Ok(());
                                },
                            }
                        },
                        Err(e) => {
                            warn!(error = %e, "Heartbeat failed, will retry next tick");
                        }
                    }
                },

                _ = message_poll_interval.tick() => {
                    match self.ams.poll_messages(&state.agent_id).await {
                        Ok(messages) => {
                            for message in messages {
                                if message.recipient != "agent" {
                                    continue;
                                }

                                if let Err(e) = self.handle_steering_message(&mut state, message).await {
                                    warn!(error = %e, agent_id = %state.agent_id, "Steering message handling failed");
                                    state.status = AgentStatus::Idle;
                                    state.current_execution = None;
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, agent_id = %state.agent_id, "Message polling failed");
                        }
                    }
                },

                // Shutdown signal (SIGTERM, SIGINT)
                _ = self.shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    state.status = AgentStatus::Dying;
                    self.execute_death_ritual(&state).await?;
                    return Ok(());
                },

                // TODO: Task execution handler
                // TODO: MCP request handler
            }
        }
    }

    async fn handle_steering_message(
        &self,
        state: &mut RuntimeState,
        message: SteeringMessage,
    ) -> Result<()> {
        let prompt = message.content_text().trim().to_string();
        if prompt.is_empty() {
            return Ok(());
        }

        let fleet_execution_id = format!("fleet-{}", Uuid::new_v4().simple());
        let requested_model = self.requested_model();
        let execution = self.ams.register_execution(&RegisterExecutionRequest {
            agent_id: state.agent_id.clone(),
            tenant_id: "default".to_string(),
            execution_id: fleet_execution_id.clone(),
            agent_name: self.config.agent.name.clone(),
            task: prompt.clone(),
            model: requested_model.clone(),
            instance_id: None,
            user_id: None,
        }).await?;

        state.status = AgentStatus::Working;
        state.current_execution = Some(execution.execution_id.clone());

        info!(
            agent_id = %state.agent_id,
            execution_id = %execution.execution_id,
            sender = %message.sender,
            message_type = %message.msg_type,
            "Processing steering message"
        );

        self.ams.emit_execution_chunk(
            &fleet_execution_id,
            &ExecutionChunkRequest {
                agent_id: state.agent_id.clone(),
                tenant_id: "default".to_string(),
                execution_id: fleet_execution_id.clone(),
                chunk_type: "start".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                data: ExecutionChunkData {
                    model: Some(requested_model.clone()),
                    ..Default::default()
                },
            },
        ).await?;

        let started_at = std::time::Instant::now();
        let system_prompt = self.hand.as_ref().and_then(|hand| hand.system_prompt.clone());

        // Check if this agent has tools enabled (team-lead class)
        // Prime/orchestrator and team-leads both need the tool loop so they
        // can dispatch, wait, and synthesize.
        let has_tools = self.hand.as_ref()
            .map(|h| matches!(
                h.manifest.hand.archetype.as_str(),
                "team-lead" | "orchestrator"
            ))
            .unwrap_or(false);

        let final_event = if has_tools {
            self.run_tool_loop(
                state, &fleet_execution_id, &prompt, &requested_model,
                system_prompt.as_deref(), started_at,
            ).await
        } else {
            // Non-TL agents: single-shot response (original behavior)
            self.run_single_shot(
                state, &fleet_execution_id, &prompt, &requested_model,
                started_at,
            ).await
        };

        self.ams.emit_execution_chunk(&fleet_execution_id, &final_event?).await?;
        state.status = AgentStatus::Idle;
        state.current_execution = None;
        Ok(())
    }

    /// Single-shot LLM response (no tools) for non-TL agents.
    async fn run_single_shot(
        &self,
        state: &mut RuntimeState,
        fleet_execution_id: &str,
        prompt: &str,
        requested_model: &str,
        started_at: std::time::Instant,
    ) -> Result<ExecutionChunkRequest> {
        let result = self.generate_response(prompt).await;
        match result {
            Ok(result) => {
                state.token_count = state.token_count
                    .saturating_add(result.input_tokens + result.output_tokens);
                state.context_pct = ((state.token_count as f64 / state.max_tokens as f64) * 100.0)
                    .clamp(0.0, 100.0);

                self.ams.emit_execution_chunk(
                    fleet_execution_id,
                    &ExecutionChunkRequest {
                        agent_id: state.agent_id.clone(),
                        tenant_id: "default".to_string(),
                        execution_id: fleet_execution_id.to_string(),
                        chunk_type: "output".to_string(),
                        timestamp: Utc::now().to_rfc3339(),
                        data: ExecutionChunkData {
                            content: Some(result.content.clone()),
                            model: Some(result.model.clone()),
                            ..Default::default()
                        },
                    },
                ).await?;

                info!(
                    agent_id = %state.agent_id,
                    provider = %result.provider,
                    model = %result.model,
                    "Steering message completed (single-shot)"
                );

                Ok(ExecutionChunkRequest {
                    agent_id: state.agent_id.clone(),
                    tenant_id: "default".to_string(),
                    execution_id: fleet_execution_id.to_string(),
                    chunk_type: "complete".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    data: ExecutionChunkData {
                        tokens_in: Some(result.input_tokens),
                        tokens_out: Some(result.output_tokens),
                        duration_ms: Some(started_at.elapsed().as_millis() as u64),
                        model: Some(result.model),
                        ..Default::default()
                    },
                })
            }
            Err(error) => {
                warn!(error = %error, "Steering message failed (single-shot)");
                Ok(ExecutionChunkRequest {
                    agent_id: state.agent_id.clone(),
                    tenant_id: "default".to_string(),
                    execution_id: fleet_execution_id.to_string(),
                    chunk_type: "error".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    data: ExecutionChunkData {
                        duration_ms: Some(started_at.elapsed().as_millis() as u64),
                        error: Some(error.to_string()),
                        model: Some(requested_model.to_string()),
                        ..Default::default()
                    },
                })
            }
        }
    }

    /// Tool-use loop for team-lead agents.
    async fn run_tool_loop(
        &self,
        state: &mut RuntimeState,
        fleet_execution_id: &str,
        prompt: &str,
        requested_model: &str,
        system_prompt: Option<&str>,
        started_at: std::time::Instant,
    ) -> Result<ExecutionChunkRequest> {
        let tools = Self::tl_tool_definitions();

        let mut messages: Vec<serde_json::Value> = Vec::new();
        if let Some(sys) = system_prompt {
            messages.push(serde_json::json!({"role": "system", "content": sys}));
        }
        messages.push(serde_json::json!({"role": "user", "content": prompt}));

        let max_iterations = 12;
        let mut total_in_tokens: u64 = 0;
        let mut total_out_tokens: u64 = 0;
        let mut final_text = String::new();

        for iteration in 0..max_iterations {
            info!(
                agent_id = %state.agent_id,
                iteration = iteration,
                messages = messages.len(),
                "Tool loop iteration"
            );

            let response = self.ams.complete_with_tools(&ToolCompletionRequest {
                messages: messages.clone(),
                tools: tools.clone(),
                max_tokens: 4000,
                model: Some(requested_model.to_string()),
                temperature: Some(0.3),
            }).await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    warn!(error = %e, iteration = iteration, "Tool loop LLM call failed");
                    return Ok(ExecutionChunkRequest {
                        agent_id: state.agent_id.clone(),
                        tenant_id: "default".to_string(),
                        execution_id: fleet_execution_id.to_string(),
                        chunk_type: "error".to_string(),
                        timestamp: Utc::now().to_rfc3339(),
                        data: ExecutionChunkData {
                            duration_ms: Some(started_at.elapsed().as_millis() as u64),
                            error: Some(e.to_string()),
                            model: Some(requested_model.to_string()),
                            ..Default::default()
                        },
                    });
                }
            };

            total_in_tokens += response.input_tokens;
            total_out_tokens += response.output_tokens;

            // If no tool calls, we're done
            if response.tool_calls.is_empty() || response.finish_reason == "stop" {
                final_text = response.text;
                info!(
                    agent_id = %state.agent_id,
                    iterations = iteration + 1,
                    "Tool loop completed"
                );
                break;
            }

            // Add assistant message with tool_calls
            let assistant_msg = serde_json::json!({
                "role": "assistant",
                "content": if response.text.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(response.text.clone()) },
                "tool_calls": response.tool_calls,
            });
            messages.push(assistant_msg);

            // Execute each tool call
            for tool_call in &response.tool_calls {
                let tc_id = tool_call.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let func = tool_call.get("function").cloned().unwrap_or_default();
                let func_name = func.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let func_args_str = func.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                let func_args: serde_json::Value = serde_json::from_str(func_args_str).unwrap_or_default();

                info!(
                    agent_id = %state.agent_id,
                    tool = func_name,
                    args = %func_args,
                    "Executing tool call"
                );

                // Emit tool use telemetry
                let _ = self.ams.emit_execution_chunk(
                    fleet_execution_id,
                    &ExecutionChunkRequest {
                        agent_id: state.agent_id.clone(),
                        tenant_id: "default".to_string(),
                        execution_id: fleet_execution_id.to_string(),
                        chunk_type: "tool_use".to_string(),
                        timestamp: Utc::now().to_rfc3339(),
                        data: ExecutionChunkData {
                            tool_name: Some(func_name.to_string()),
                            tool_input: Some(func_args.clone()),
                            ..Default::default()
                        },
                    },
                ).await;

                let tool_result = self.execute_tool(func_name, &func_args, &state.agent_id).await;

                info!(
                    agent_id = %state.agent_id,
                    tool = func_name,
                    result_len = tool_result.len(),
                    "Tool call completed"
                );

                // Emit tool result telemetry
                let _ = self.ams.emit_execution_chunk(
                    fleet_execution_id,
                    &ExecutionChunkRequest {
                        agent_id: state.agent_id.clone(),
                        tenant_id: "default".to_string(),
                        execution_id: fleet_execution_id.to_string(),
                        chunk_type: "tool_result".to_string(),
                        timestamp: Utc::now().to_rfc3339(),
                        data: ExecutionChunkData {
                            tool_name: Some(func_name.to_string()),
                            tool_output: Some(tool_result.clone()),
                            ..Default::default()
                        },
                    },
                ).await;

                // Add tool result message
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tc_id,
                    "content": tool_result,
                }));
            }
        }

        state.token_count = state.token_count.saturating_add(total_in_tokens + total_out_tokens);
        state.context_pct = ((state.token_count as f64 / state.max_tokens as f64) * 100.0)
            .clamp(0.0, 100.0);

        // Emit final output
        if !final_text.is_empty() {
            let _ = self.ams.emit_execution_chunk(
                fleet_execution_id,
                &ExecutionChunkRequest {
                    agent_id: state.agent_id.clone(),
                    tenant_id: "default".to_string(),
                    execution_id: fleet_execution_id.to_string(),
                    chunk_type: "output".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    data: ExecutionChunkData {
                        content: Some(final_text.clone()),
                        model: Some(requested_model.to_string()),
                        ..Default::default()
                    },
                },
            ).await;

            // Persist the synthesized orchestration result as an episodic
            // memory on the caller. Makes the dashboard "recent memories"
            // panel show the actual fan-in output, and lets subsequent
            // hybrid searches find it next turn.
            let mem = abot_ams::memory::CreateMemoryRequest {
                title: format!("{}: orchestration result", state.agent_id),
                content: final_text.clone(),
                memory_tier: "episodic".to_string(),
                entity_type: "event".to_string(),
                importance: 0.7,
                tags: vec![
                    "orchestration".to_string(),
                    "fan-in".to_string(),
                    format!("agent:{}", state.agent_id),
                    format!("exec:{}", fleet_execution_id),
                ],
                metadata: Some(serde_json::json!({
                    "source_agent": state.agent_id,
                    "fleet_execution_id": fleet_execution_id,
                })),
            };
            if let Err(e) = self.ams.create_memory(mem).await {
                warn!(
                    agent_id = %state.agent_id,
                    execution_id = %fleet_execution_id,
                    error = %e,
                    "Failed to persist orchestration result as memory"
                );
            }
        }

        Ok(ExecutionChunkRequest {
            agent_id: state.agent_id.clone(),
            tenant_id: "default".to_string(),
            execution_id: fleet_execution_id.to_string(),
            chunk_type: "complete".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            data: ExecutionChunkData {
                tokens_in: Some(total_in_tokens),
                tokens_out: Some(total_out_tokens),
                duration_ms: Some(started_at.elapsed().as_millis() as u64),
                model: Some(requested_model.to_string()),
                ..Default::default()
            },
        })
    }

    /// Execute a tool call and return the result string.
    async fn execute_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
        caller_agent_id: &str,
    ) -> String {
        match name {
            "dispatch_to_worker" => {
                let worker = args.get("worker_name").and_then(|v| v.as_str()).unwrap_or("");
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(180);
                if worker.is_empty() || task.is_empty() {
                    return serde_json::json!({"error": "worker_name and task are required"}).to_string();
                }

                // Step 1: dispatch. AMS now returns execution_id when
                // spawn_triggered=true (paired with agent-memory-backend
                // commit fdcf223).
                let dispatch = match self.ams.send_steering_message(worker, task, "task", caller_agent_id).await {
                    Ok(v) => v,
                    Err(e) => return serde_json::json!({
                        "ok": false, "error": format!("dispatch: {}", e),
                    }).to_string(),
                };

                let exec_id = match dispatch.get("execution_id").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => {
                        // Worker was already alive; message just queued.
                        // Nothing fresh to wait on.
                        return serde_json::json!({
                            "ok": true,
                            "dispatched_to": worker,
                            "status": "enqueued_only",
                            "note": "worker was alive; message queued but no new execution",
                            "response": dispatch,
                        }).to_string();
                    }
                };

                // Step 2: poll the observatory until terminal, then return
                // the output so the orchestrator can synthesize.
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_secs(timeout_secs);
                let poll_interval = std::time::Duration::from_secs(2);

                loop {
                    if std::time::Instant::now() > deadline {
                        return serde_json::json!({
                            "ok": false,
                            "dispatched_to": worker,
                            "execution_id": exec_id,
                            "status": "timeout",
                            "timeout_secs": timeout_secs,
                        }).to_string();
                    }
                    match self.ams.get_execution(&exec_id).await {
                        Ok(exec) => {
                            let status = exec.get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            if matches!(status, "completed" | "failed" | "killed") {
                                let output = exec.get("output").cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                return serde_json::json!({
                                    "ok": status == "completed",
                                    "dispatched_to": worker,
                                    "execution_id": exec_id,
                                    "status": status,
                                    "output": output,
                                    "duration_ms": exec.get("duration_ms"),
                                }).to_string();
                            }
                        }
                        Err(e) => {
                            // 404s are expected for the first few polls
                            // while the background spawn writes the row.
                            tracing::debug!(exec_id = %exec_id, err = %e, "get_execution transient");
                        }
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            "search_memories" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                match self.ams.search_memories(query, limit).await {
                    Ok(memories) => {
                        let summaries: Vec<serde_json::Value> = memories.iter().map(|m| {
                            serde_json::json!({
                                "title": m.title,
                                "content": &m.content[..m.content.len().min(200)],
                                "tags": m.tags,
                            })
                        }).collect();
                        serde_json::json!({"results": summaries}).to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "list_workers" => {
                match self.ams.list_worker_agents().await {
                    Ok(agents) => {
                        let names: Vec<String> = agents.iter().filter_map(|a| {
                            a.get("agent_name").and_then(|v| v.as_str()).map(|s| s.to_string())
                        }).collect();
                        serde_json::json!({"workers": names}).to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            _ => {
                serde_json::json!({"error": format!("Unknown tool: {}", name)}).to_string()
            }
        }
    }

    /// Tool definitions for team-lead agents.
    fn tl_tool_definitions() -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "dispatch_to_worker",
                    "description": "Dispatch a subtask to a worker agent. The worker will execute the task in a git worktree and produce a PR.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "worker_name": {
                                "type": "string",
                                "description": "Name of the worker agent (e.g. backend-engineer, frontend-engineer, coder, researcher, technical-writer)"
                            },
                            "task": {
                                "type": "string",
                                "description": "Detailed task description for the worker. Include context, requirements, and expected deliverables."
                            }
                        },
                        "required": ["worker_name", "task"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "search_memories",
                    "description": "Search AMS memories for relevant domain knowledge, past decisions, and context.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Search query to find relevant memories"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Max results to return (default 5)",
                                "default": 5
                            }
                        },
                        "required": ["query"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "list_workers",
                    "description": "List all available worker agents that can be dispatched to.",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            }),
        ]
    }

    async fn generate_response(&self, prompt: &str) -> Result<GenerationResult> {
        let system_prompt = self.hand.as_ref().and_then(|hand| hand.system_prompt.clone());

        if let Some(bridge) = self.kilo_bridge() {
            let mode = self.kilo_mode();
            let prompt_for_kilo = if let Some(system) = &system_prompt {
                format!("{system}\n\nUser request:\n{prompt}")
            } else {
                prompt.to_string()
            };

            let response = tokio::task::spawn_blocking(move || bridge.execute(&prompt_for_kilo, mode))
                .await??;

            return Ok(GenerationResult {
                content: response.content,
                model: response.model_used,
                provider: "kilo_local".to_string(),
                input_tokens: 0,
                output_tokens: response.tokens_used,
            });
        }

        let requested_model = self.requested_model();
        let response = self.ams.complete(&CompletionRequest {
            prompt: prompt.to_string(),
            max_tokens: 4000,
            role: "agent".to_string(),
            model: Some(requested_model),
            system_prompt,
            temperature: None,
        }).await?;

        Ok(GenerationResult {
            content: response.text,
            model: response.model,
            provider: response.provider,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    fn requested_model(&self) -> String {
        if let Some(hand) = &self.hand {
            if !hand.manifest.hand.default_model.trim().is_empty() {
                return hand.manifest.hand.default_model.trim().to_string();
            }
        }

        match self.config.llm.provider {
            crate::config::LlmProvider::Kilo => "kilo".to_string(),
            crate::config::LlmProvider::Direct => "ams-agent".to_string(),
        }
    }

    fn kilo_bridge(&self) -> Option<KiloBridge> {
        if !matches!(self.config.llm.provider, crate::config::LlmProvider::Kilo) {
            return None;
        }

        let kilo_path = self
            .config
            .llm
            .kilo
            .as_ref()
            .map(|cfg| cfg.binary.clone())
            .unwrap_or_else(|| "kilo".to_string());

        let path = std::path::Path::new(&kilo_path);
        if (path.is_absolute() && path.exists()) || which::which(&kilo_path).is_ok() {
            Some(KiloBridge::new(Some(kilo_path)))
        } else {
            None
        }
    }

    fn kilo_mode(&self) -> KiloMode {
        let raw_mode = self
            .config
            .llm
            .kilo
            .as_ref()
            .map(|cfg| cfg.default_mode.as_str())
            .unwrap_or("code");

        match raw_mode {
            "architect" => KiloMode::Architect,
            "debug" => KiloMode::Debug,
            "ask" => KiloMode::Ask,
            "orchestrator" => KiloMode::Orchestrator,
            _ => KiloMode::Code,
        }
    }

    /// Execute death ritual: save memories, create continuation, exit.
    async fn execute_death_ritual(&self, state: &RuntimeState) -> Result<()> {
        info!(
            agent_id = %state.agent_id,
            context_pct = state.context_pct,
            "Executing death ritual"
        );

        // AMS handles all the intelligence:
        // - Saving memories
        // - Creating continuation with next_action
        // - Updating governance FSM
        // - Fleet coordination
        let _death_response = self.ams.death(abot_ams::warden::DeathRequest {
            agent_id: state.agent_id.clone(),
            original_goal: String::new(), // TODO: track current goal
            next_action: String::new(),    // TODO: determine next action
            completed_subtasks: vec![],    // TODO: track subtasks
            remaining_subtasks: vec![],
            handoff_notes: None,
            memories: vec![],              // TODO: crystallize session memories
            context_pct: state.context_pct,
        }).await?;

        info!("Death ritual complete. Goodbye.");
        Ok(())
    }
}
