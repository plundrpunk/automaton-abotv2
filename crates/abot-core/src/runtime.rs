use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::config::AbotConfig;
use crate::hand::{LoadedHand, load_hand};
use abot_ams::client::{AmsClient, AmsConfig, SteeringMessage};
use abot_ams::fleet::{ExecutionChunkData, ExecutionChunkRequest, FleetRegisterAgentRequest, RegisterExecutionRequest};
use abot_ams::llm::{CompletionRequest, ToolCompletionRequest};
use abot_ams::warden::{AmsGrants, BirthRequest, Directive};
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
    grants: Option<AmsGrants>,
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
            grants: None,
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

        // Store and log AMS-granted operating limits
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
            self.grants = Some(grants.clone());
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

        // === FLEET REGISTRATION ===
        // Idempotent one-time registration into the in-memory
        // fleet_registered_agents map so `/api/fleet/status` and the
        // dashboards see this container. Fail-open: fleet is observability,
        // not lifecycle, so we never propagate errors here.
        let fleet_metadata = serde_json::json!({
            "version": "3.0.0",
            "runtime": "rust",
            "sandbox": self.config.sandbox.engine,
            "container_id": self.heartbeat.container_id(),
        });
        let fleet_register_req = FleetRegisterAgentRequest {
            agent_id: self.config.agent.id.clone(),
            tenant_id: Some(self.heartbeat.tenant_id().to_string()),
            agent_name: Some(self.config.agent.name.clone()),
            instance_id: Some(self.heartbeat.container_id().to_string()),
            metadata: fleet_metadata,
        };
        match self.ams.fleet_register_agent(&fleet_register_req).await {
            Ok(resp) => info!(
                agent_id = %self.config.agent.id,
                container_id = %self.heartbeat.container_id(),
                registered_at = ?resp.registered_at,
                "Fleet registration complete"
            ),
            Err(e) => warn!(
                agent_id = %self.config.agent.id,
                error = %e,
                "Fleet registration failed — continuing (heartbeat will retry via /api/fleet/heartbeat upsert)"
            ),
        }

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

        // Tools are enabled either by archetype (team-leads and orchestrators
        // always get the dispatch/wait/synthesize loop) or by AMS birth grants
        // (enable_tools=true opts any agent into the tool loop).
        let archetype = self.hand.as_ref()
            .map(|h| h.manifest.hand.archetype.as_str())
            .unwrap_or("");
        let archetype_enables_tools = matches!(archetype, "team-lead" | "orchestrator");
        let grants_enable_tools = self.grants.as_ref()
            .map(|g| g.enable_tools)
            .unwrap_or(false);
        let has_tools = archetype_enables_tools || grants_enable_tools;

        // If this activation was spawned by a parent orchestrator's
        // dispatch_to_tl, capture the rollup breadcrumbs so the tool loop
        // can ding the parent when it finishes. For msg_type == "rollup"
        // arriving at an orchestrator, we're the terminus (rollup_target
        // stays None) and the prompt is augmented instead so the LLM
        // knows to synthesize the TL's result for the user.
        let incoming_meta = message.metadata.as_ref();
        // chat_session_id rides on any msg_type originating from a
        // dashboard-initiated turn. We pluck it once here and thread it
        // through run_tool_loop; on completion the runtime posts its
        // synthesis back to that dashboard chat session.
        let chat_session_id_owned: Option<String> = incoming_meta
            .and_then(|m| m.get("chat_session_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let (rollup_target_owned, prompt) = match message.msg_type.as_str() {
            "task" => {
                let parent = incoming_meta
                    .and_then(|m| m.get("parent_agent_id"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let parent_exec = incoming_meta
                    .and_then(|m| m.get("parent_exec_id"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let target = match (parent, parent_exec) {
                    (Some(a), Some(e)) => Some((a, e)),
                    _ => None,
                };
                (target, prompt)
            }
            "rollup" => {
                let child_agent = incoming_meta
                    .and_then(|m| m.get("child_agent_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(message.sender.as_str())
                    .to_string();
                let child_exec = incoming_meta
                    .and_then(|m| m.get("child_exec_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let memory_id = incoming_meta
                    .and_then(|m| m.get("memory_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let augmented = format!(
                    "[rollup from team-lead {child_agent}, exec {child_exec}, memory {memory_id}]\n\n{prompt}\n\nSynthesize this result for the user."
                );
                (None, augmented)
            }
            _ => (None, prompt),
        };
        let rollup_target = rollup_target_owned
            .as_ref()
            .map(|(a, e)| (a.as_str(), e.as_str()));

        let chat_session_id = chat_session_id_owned.as_deref();
        let final_event = if has_tools {
            self.run_tool_loop(
                state, &fleet_execution_id, &prompt, &requested_model,
                system_prompt.as_deref(), started_at, rollup_target,
                chat_session_id,
            ).await
        } else {
            // Non-tooled agents: single-shot response (original behavior)
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

    /// Tool-use loop for agents with tools enabled (TLs, Prime, etc).
    ///
    /// `rollup_target`, when `Some((parent_agent_id, parent_exec_id))`,
    /// means this activation was spawned by a higher-level orchestrator's
    /// `dispatch_to_tl`. On completion, after persisting the
    /// orchestration-result memory, we POST a `rollup` steering back to
    /// the parent so it gets a ding instead of having to poll.
    ///
    /// `chat_session_id`, when `Some`, means this activation ultimately
    /// traces back to a dashboard chat turn. After the tool loop ends we
    /// also POST the final assistant text to that chat session so the
    /// user sees the response in the conversation that initiated the
    /// work — even when this turn is a rollup-triggered synthesis
    /// rather than the original user-facing turn.
    ///
    /// TODO(fan-in-aware): today this is "next-idle" - the rollup lands in
    /// the parent's warden queue and is consumed on its next message-poll
    /// tick. Eventually the parent's tool loop should track outstanding
    /// child exec_ids and fast-path-unblock a waiting dispatch_and_wait
    /// when a matching rollup arrives, instead of the dispatch_to_worker
    /// polling path we have today.
    async fn run_tool_loop(
        &self,
        state: &mut RuntimeState,
        fleet_execution_id: &str,
        prompt: &str,
        requested_model: &str,
        system_prompt: Option<&str>,
        started_at: std::time::Instant,
        rollup_target: Option<(&str, &str)>,
        chat_session_id: Option<&str>,
    ) -> Result<ExecutionChunkRequest> {
        let archetype = self.hand.as_ref()
            .map(|h| h.manifest.hand.archetype.as_str())
            .unwrap_or("");
        let tools = match archetype {
            "team-lead" => {
                let prefix = Self::tl_specialist_prefix(&state.agent_id);
                let specialists = match self
                    .ams
                    .list_worker_agents_filtered(prefix.as_deref())
                    .await
                {
                    Ok(list) => list,
                    Err(e) => {
                        warn!(
                            agent_id = %state.agent_id,
                            prefix = ?prefix,
                            error = %e,
                            "Failed to load domain specialist roster; falling back to empty list"
                        );
                        Vec::new()
                    }
                };
                info!(
                    agent_id = %state.agent_id,
                    prefix = ?prefix,
                    specialist_count = specialists.len(),
                    "Loaded domain specialist roster for TL tool schema"
                );
                Self::tl_tool_definitions(&specialists)
            }
            _ => Self::orchestrator_tool_definitions(),
        };

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

                let tool_result = self.execute_tool(func_name, &func_args, &state.agent_id, Some(fleet_execution_id), chat_session_id).await;

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
            let memory_id = match self.ams.create_memory(mem).await {
                Ok(resp) => resp
                    .get("id")
                    .or_else(|| resp.get("memory_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                Err(e) => {
                    warn!(
                        agent_id = %state.agent_id,
                        execution_id = %fleet_execution_id,
                        error = %e,
                        "Failed to persist orchestration result as memory"
                    );
                    None
                }
            };

            // Roll the result back up to the orchestrator that dispatched
            // us. Additive to the memory write: memory is the durable audit
            // record, this steering is the live delivery so the parent
            // gets a ding on next-idle rather than having to poll.
            if let Some((parent_agent, parent_exec)) = rollup_target {
                let summary: String = if final_text.len() > 500 {
                    let mut end = 500;
                    while !final_text.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &final_text[..end])
                } else {
                    final_text.clone()
                };
                let mut rollup_meta = serde_json::json!({
                    "parent_exec_id": parent_exec,
                    "child_exec_id": fleet_execution_id,
                    "child_agent_id": state.agent_id,
                    "memory_id": memory_id.clone().unwrap_or_default(),
                });
                // Thread the dashboard chat session all the way back up so
                // the parent's rollup-triggered synthesis turn can post
                // its reply into the same conversation.
                if let Some(cs) = chat_session_id {
                    rollup_meta["chat_session_id"] = serde_json::Value::String(cs.to_string());
                }
                if let Err(e) = self.ams.send_steering_message(
                    parent_agent,
                    &summary,
                    "rollup",
                    &state.agent_id,
                    Some(&rollup_meta),
                ).await {
                    warn!(
                        agent_id = %state.agent_id,
                        parent_agent = %parent_agent,
                        parent_exec = %parent_exec,
                        error = %e,
                        "Failed to emit rollup steering; parent will rely on memory read",
                    );
                } else {
                    info!(
                        agent_id = %state.agent_id,
                        parent_agent = %parent_agent,
                        parent_exec = %parent_exec,
                        memory_id = ?memory_id,
                        "Rolled up orchestration result to parent",
                    );
                }
            }
        }

        // Dashboard writeback: if this activation came from a dashboard
        // chat turn (either the original user message or a rollup that
        // inherited the chat_session_id), post the final synthesized
        // assistant text into that chat session so it shows up in the
        // conversation the user started. Best-effort — a failure here
        // doesn't break the tool-loop result.
        if let Some(cs) = chat_session_id {
            if !final_text.trim().is_empty() {
                if let Err(e) = self.ams.post_chat_message(
                    cs,
                    &final_text,
                    &state.agent_id,
                    Some(requested_model),
                ).await {
                    warn!(
                        agent_id = %state.agent_id,
                        execution_id = %fleet_execution_id,
                        chat_session_id = %cs,
                        error = %e,
                        "Failed to post synthesis back to dashboard chat session",
                    );
                } else {
                    info!(
                        agent_id = %state.agent_id,
                        execution_id = %fleet_execution_id,
                        chat_session_id = %cs,
                        "Posted synthesis to dashboard chat session",
                    );
                }
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
    ///
    /// `caller_exec_id` is this agent's current fleet execution id. Tools
    /// that dispatch work downstream (e.g. `dispatch_to_tl`) include it in
    /// their steering-message metadata so the recipient knows who to roll
    /// back up to.
    ///
    /// `chat_session_id`, when `Some`, is the dashboard chat session that
    /// originated this turn. Dispatch tools propagate it in the outbound
    /// steering metadata so a downstream rollup can eventually land back
    /// in the same conversation.
    async fn execute_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
        caller_agent_id: &str,
        caller_exec_id: Option<&str>,
        chat_session_id: Option<&str>,
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
                let dispatch = match self.ams.send_steering_message(worker, task, "task", caller_agent_id, None).await {
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
            "dispatch_to_tl" => {
                let tl_name = args.get("tl_name").and_then(|v| v.as_str()).unwrap_or("");
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("normal");
                if tl_name.is_empty() || task.is_empty() {
                    return serde_json::json!({"error": "tl_name and task are required"}).to_string();
                }
                // Hand the TL the rollup breadcrumbs: our agent_id + exec_id.
                // When the TL's orchestration loop ends, it POSTs a `rollup`
                // steering back to us tagged with parent_exec_id so we can
                // correlate on next-idle activation. If our turn came from
                // a dashboard chat session, we also pass that through so
                // the TL's rollup carries it back up and our eventual
                // synthesis turn can post into the same conversation.
                let rollup_meta = caller_exec_id.map(|exec| {
                    let mut m = serde_json::json!({
                        "parent_agent_id": caller_agent_id,
                        "parent_exec_id": exec,
                    });
                    if let Some(cs) = chat_session_id {
                        m["chat_session_id"] = serde_json::Value::String(cs.to_string());
                    }
                    m
                });
                match self.ams.send_steering_message(tl_name, task, "task", caller_agent_id, rollup_meta.as_ref()).await {
                    Ok(resp) => serde_json::json!({
                        "ok": true,
                        "dispatched_to": tl_name,
                        "priority": priority,
                        "response": resp,
                    }).to_string(),
                    Err(e) => serde_json::json!({
                        "ok": false,
                        "error": e.to_string(),
                    }).to_string(),
                }
            }
            "list_tl_agents" => {
                match self.ams.list_worker_agents().await {
                    Ok(agents) => {
                        let tls: Vec<serde_json::Value> = agents.iter().filter_map(|a| {
                            // /api/v1/agents returns `agent_id` (canonical slug).
                            let name = a.get("agent_id").and_then(|v| v.as_str())?;
                            if name.starts_with("tl-") {
                                Some(serde_json::json!({
                                    "name": name,
                                    "trust_tier": a.get("trust_tier").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                    "automata_count": a.get("automata_count").and_then(|v| v.as_u64()).unwrap_or(0),
                                }))
                            } else {
                                None
                            }
                        }).collect();
                        serde_json::json!({"team_leads": tls, "count": tls.len()}).to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "create_goal_task" => {
                let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
                let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("normal");
                if title.is_empty() {
                    return serde_json::json!({"error": "title is required"}).to_string();
                }
                match self.ams.create_goal_task(title, description, priority, caller_agent_id).await {
                    Ok(resp) => resp,
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "search_memories" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                match self.ams.search_memories(query, limit).await {
                    Ok(results) => {
                        let summaries: Vec<serde_json::Value> = results.iter().map(|r| {
                            let memory = r.get("memory").cloned().unwrap_or(serde_json::Value::Null);
                            let file_path = memory.get("file_path")
                                .and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let tags = memory.get("tags").cloned().unwrap_or(serde_json::Value::Array(vec![]));
                            let snippet = r.get("content_snippet")
                                .and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let score = r.get("relevance_score")
                                .and_then(|v| v.as_f64()).unwrap_or(0.0);
                            serde_json::json!({
                                "file_path": file_path,
                                "tags": tags,
                                "snippet": if snippet.len() > 200 {
                                    let mut end = 200;
                                    while !snippet.is_char_boundary(end) { end -= 1; }
                                    snippet[..end].to_string()
                                } else { snippet },
                                "score": score,
                            })
                        }).collect();
                        serde_json::json!({"results": summaries}).to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "list_workers" => {
                // Scope to this TL's domain-prefixed specialists. If we're
                // not a TL (no tl- prefix), fall back to the unscoped
                // specialist roster minus TL daemons + curator.
                let prefix = Self::tl_specialist_prefix(caller_agent_id);
                let result = match &prefix {
                    Some(p) => self.ams.list_worker_agents_filtered(Some(p)).await,
                    None => self.ams.list_worker_agents().await,
                };
                match result {
                    Ok(agents) => {
                        let names: Vec<String> = agents.iter().filter_map(|a| {
                            let id = a.get("agent_id").and_then(|v| v.as_str())?;
                            if id.starts_with("tl-") || id == "memory-curator" {
                                None
                            } else {
                                Some(id.to_string())
                            }
                        }).collect();
                        serde_json::json!({
                            "workers": names,
                            "count": names.len(),
                            "scope_prefix": prefix,
                        }).to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            _ => {
                serde_json::json!({"error": format!("Unknown tool: {}", name)}).to_string()
            }
        }
    }

    /// Derive the specialist agent-name prefix from a TL agent_id.
    ///
    /// `tl-engineering` -> `engineering-`, `tl-paid-media` -> `paid-media-`,
    /// etc. Returns `None` for non-TL ids so callers can fall back to the
    /// unfiltered roster.
    fn tl_specialist_prefix(agent_id: &str) -> Option<String> {
        let rest = agent_id.strip_prefix("tl-")?;
        if rest.is_empty() {
            None
        } else {
            Some(format!("{}-", rest))
        }
    }

    /// Tool definitions for team-lead agents.
    ///
    /// `specialists` is the domain-scoped roster (agent rows returned by AMS
    /// `/api/v1/agents?name_prefix=...`). We use it to constrain the
    /// `dispatch_to_worker.worker_name` argument to a real enum of agents
    /// that actually exist in the agents table, so the LLM cannot hallucinate
    /// a name like "coder" that would fail downstream spawn.
    fn tl_tool_definitions(specialists: &[serde_json::Value]) -> Vec<serde_json::Value> {
        let specialist_names: Vec<String> = specialists
            .iter()
            .filter_map(|a| {
                a.get("agent_id")
                    .or_else(|| a.get("name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let roster_desc = if specialist_names.is_empty() {
            "No domain specialists are registered for you. Report this to the caller.".to_string()
        } else {
            let bullets: Vec<String> = specialists
                .iter()
                .filter_map(|a| {
                    let name = a.get("agent_id").or_else(|| a.get("name")).and_then(|v| v.as_str())?;
                    let desc = a.get("description").and_then(|v| v.as_str()).unwrap_or("").trim();
                    if desc.is_empty() {
                        Some(format!("- {}", name))
                    } else {
                        let short = if desc.len() > 160 {
                            let mut end = 160;
                            while !desc.is_char_boundary(end) { end -= 1; }
                            format!("{}...", &desc[..end])
                        } else {
                            desc.to_string()
                        };
                        Some(format!("- {}: {}", name, short))
                    }
                })
                .collect();
            format!(
                "Available specialists (domain-scoped). Pick the one whose role best fits the task. Do NOT invent names:\n{}",
                bullets.join("\n"),
            )
        };

        let worker_name_schema = if specialist_names.is_empty() {
            serde_json::json!({
                "type": "string",
                "description": roster_desc,
            })
        } else {
            serde_json::json!({
                "type": "string",
                "enum": specialist_names,
                "description": roster_desc,
            })
        };

        vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "dispatch_to_worker",
                    "description": "Dispatch a subtask to one of your domain specialists. The worker will execute the task and produce a result.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "worker_name": worker_name_schema,
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
                    "description": "List your domain specialists that can be dispatched to. Scoped to this TL only.",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            }),
        ]
    }

    /// Tool definitions for orchestrator agents (Prime, etc).
    /// These dispatch to TLs rather than workers.
    fn orchestrator_tool_definitions() -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "dispatch_to_tl",
                    "description": "Dispatch a task to a Team Lead agent. The TL will route it to appropriate specialist workers in their domain. Use this to delegate work — you orchestrate, they execute.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "tl_name": {
                                "type": "string",
                                "description": "Team Lead agent name (e.g. tl-engineering, tl-marketing, tl-product, tl-design, tl-sales, tl-gamedev, tl-academic, tl-paid-media, tl-project-mgmt, tl-spatial, tl-support, tl-testing, tl-specialized)"
                            },
                            "task": {
                                "type": "string",
                                "description": "Detailed task description. Include context, requirements, and expected deliverables."
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["low", "normal", "high", "urgent"],
                                "description": "Task priority level (default: normal)"
                            }
                        },
                        "required": ["tl_name", "task"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "list_tl_agents",
                    "description": "List all available Team Lead agents and their current status.",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "create_goal_task",
                    "description": "Create a goal/task for DLPFC to route via NEXUS. Use this for complex multi-domain tasks that need intelligent routing rather than direct TL dispatch.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Short task title"
                            },
                            "description": {
                                "type": "string",
                                "description": "Detailed task description with context and requirements"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["low", "normal", "high", "urgent"],
                                "description": "Task priority (default: normal)"
                            }
                        },
                        "required": ["title"]
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
