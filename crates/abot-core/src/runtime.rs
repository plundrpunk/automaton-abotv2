use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::AbotConfig;
use crate::hand::{LoadedHand, load_hand};
use abot_ams::client::{AmsClient, AmsConfig, SteeringMessage};
use abot_ams::fleet::{
    ExecutionChunkData, ExecutionChunkRequest, FleetRegisterAgentRequest, RegisterExecutionRequest,
};
use abot_ams::llm::{CompletionRequest, ToolCompletionRequest};
use abot_ams::warden::{AmsGrants, BirthRequest, Directive};
use abot_llm::KiloBridge;
use abot_llm::kilo::KiloMode;
use abot_telemetry::heartbeat::{HeartbeatReporter, RuntimeState as TelemetryState};

/// Internal struct to hold owned data across async boundaries
/// before mapping to the borrowed `ExecutionChunkRequest` for serialization.
#[derive(Debug, Default)]
struct ExecutionCompleteEvent {
    pub content: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub model: Option<String>,
    pub chunk_type: &'static str,
}

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

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Booting => "booting",
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Dying => "dying",
        }
    }
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

fn truncate_chars(value: &str, max_chars: usize) -> &str {
    match value.char_indices().nth(max_chars) {
        Some((idx, _)) => &value[..idx],
        None => value,
    }
}

/// Upper bound on a single `poll_worker_execution` wait, so one fan-in
/// call cannot eat a whole turn.
const MAX_POLL_WAIT_SECS: u64 = 600;

/// Observatory statuses that mean the execution row will not change again.
///
/// `error` and `timeout` are terminal on the AMS side (see
/// `dlpfc_service._dispatch_monitor_loop`), so a poller that only watches
/// for `completed | failed | killed` sits on a dead row until its own
/// deadline expires.
fn is_terminal_exec_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "failed" | "killed" | "error" | "timeout"
    )
}

/// Shape a terminal observatory row into a `dispatch_to_worker` /
/// `poll_worker_execution` result.
fn terminal_worker_result(
    worker: Option<&str>,
    exec_id: &str,
    status: &str,
    exec: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "ok": status == "completed",
        "dispatched_to": worker,
        "execution_id": exec_id,
        "status": status,
        "terminal": true,
        "output": terminal_output(exec),
        "duration_ms": exec.get("duration_ms"),
    })
}

/// Shape a non-terminal observatory row.
///
/// A wait that runs out is NOT a failed dispatch: the worker keeps running
/// and its output lands on the same execution row. Return whatever partial
/// output exists plus the exact call that fans the result in on a later
/// turn, so the caller can honour "re-check on your next turn" instead of
/// re-dispatching work that is already in flight.
fn pending_worker_result(
    worker: Option<&str>,
    exec_id: &str,
    observed_status: &str,
    waited_secs: u64,
    exec: Option<&serde_json::Value>,
) -> serde_json::Value {
    let partial = exec
        .map(terminal_output)
        .as_ref()
        .and_then(|v| v.as_str())
        .map(summarize_rollup_text);

    serde_json::json!({
        "ok": false,
        "dispatched_to": worker,
        "execution_id": exec_id,
        "status": "still_running",
        "terminal": false,
        "observed_status": observed_status,
        "waited_secs": waited_secs,
        "partial_output": partial,
        "note": concat!(
            "Wait window elapsed; the worker is still running and its full ",
            "output will land on this execution row. This is not a failure ",
            "and not a lost result - do not re-dispatch this task.",
        ),
        "resume_with": {
            "tool": "poll_worker_execution",
            "arguments": { "execution_id": exec_id },
        },
    })
}

fn summarize_rollup_text(value: &str) -> String {
    let truncated = truncate_chars(value, 500);

    if truncated.len() < value.len() {
        format!("{truncated}...")
    } else {
        value.to_string()
    }
}

fn metadata_string(meta: Option<&serde_json::Value>, keys: &[&str]) -> Option<String> {
    let meta = meta?;
    for key in keys {
        if let Some(value) = meta.get(*key).and_then(|v| v.as_str())
            && !value.is_empty()
        {
            return Some(value.to_string());
        }
    }
    None
}

fn execution_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("execution_id")
        .or_else(|| value.get("executionId"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn terminal_output(value: &serde_json::Value) -> serde_json::Value {
    value
        .get("output")
        .or_else(|| value.get("output_buffer"))
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

fn normalize_task_status(status: &str) -> Option<&'static str> {
    match status
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "pending" | "todo" | "to_do" | "backlog" | "open" | "queued" => Some("pending"),
        "active" | "in_progress" | "running" | "working" | "started" => Some("active"),
        "done" | "complete" | "completed" | "succeeded" | "closed" => Some("done"),
        "failed" | "error" | "errored" | "killed" | "cancelled" | "canceled" => Some("failed"),
        _ => None,
    }
}

fn task_title(value: &serde_json::Value) -> Option<String> {
    value
        .get("title")
        .or_else(|| value.get("name"))
        .or_else(|| value.get("task"))
        .or_else(|| value.get("description"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.chars().count() > 120 {
                format!("{}...", s.chars().take(120).collect::<String>())
            } else {
                s.to_string()
            }
        })
}

fn collect_task_board_items(
    value: &serde_json::Value,
    inherited_status: Option<&'static str>,
    out: &mut Vec<(String, String)>,
) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_task_board_items(item, inherited_status, out);
            }
        }
        serde_json::Value::Object(map) => {
            let own_status = map
                .get("status")
                .or_else(|| map.get("state"))
                .and_then(|v| v.as_str())
                .and_then(normalize_task_status)
                .or(inherited_status);

            if let (Some(status), Some(title)) = (own_status, task_title(value)) {
                out.push((status.to_string(), title));
            }

            for (key, child) in map {
                let key_status = normalize_task_status(key).or(own_status);
                collect_task_board_items(child, key_status, out);
            }
        }
        _ => {}
    }
}

fn task_board_summary(board: &serde_json::Value) -> serde_json::Value {
    let mut items = Vec::new();
    collect_task_board_items(board, None, &mut items);

    let mut pending = 0usize;
    let mut active = 0usize;
    let mut done = 0usize;
    let mut failed = 0usize;
    let mut top_tasks = Vec::new();

    for (status, title) in items {
        match status.as_str() {
            "pending" => pending += 1,
            "active" => active += 1,
            "done" => done += 1,
            "failed" => failed += 1,
            _ => {}
        }
        if top_tasks.len() < 10 {
            top_tasks.push(serde_json::json!({
                "status": status,
                "title": title,
            }));
        }
    }

    serde_json::json!({
        "counts": {
            "pending": pending,
            "active": active,
            "done": done,
            "failed": failed,
        },
        "top_tasks": top_tasks,
    })
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
        let heartbeat = HeartbeatReporter::new(ams.clone(), config.ams.heartbeat_interval_secs);

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
            if let (Some(base), Some(extra)) = (birth_metadata.as_object_mut(), claims.as_object())
            {
                for (k, v) in extra {
                    base.insert(k.clone(), v.clone());
                }
            }
            if hand.system_prompt.is_some() {
                birth_metadata["has_system_prompt"] = serde_json::json!(true);
            }
        }

        let birth_response = self
            .ams
            .birth(BirthRequest {
                agent_id: self.config.agent.id.clone(),
                agent_name: self.config.agent.name.clone(),
                metadata: birth_metadata,
            })
            .await?;

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
            agent_id: &self.config.agent.id,
            tenant_id: Some(self.heartbeat.tenant_id()),
            agent_name: Some(self.config.agent.name.as_str()),
            instance_id: Some(self.heartbeat.container_id()),
            metadata: &fleet_metadata,
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
        let mut heartbeat_interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.ams.heartbeat_interval_secs,
        ));
        let mut message_poll_interval = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            // Convert core RuntimeState → telemetry RuntimeState for heartbeat
            let telemetry_state = TelemetryState {
                agent_id: &state.agent_id,
                context_pct: state.context_pct,
                status: state.status.as_str(),
                current_execution: state.current_execution.as_deref(),
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
        let incoming_meta = message.metadata.as_ref();
        let requested_model = self.requested_model();
        let execution = self
            .ams
            .register_execution(&RegisterExecutionRequest {
                agent_id: &state.agent_id,
                tenant_id: "default",
                execution_id: &fleet_execution_id,
                agent_name: &self.config.agent.name,
                task: &prompt,
                model: &requested_model,
                instance_id: None,
                user_id: None,
                parent_orchestration_id: metadata_string(
                    incoming_meta,
                    &["parent_orchestration_id"],
                ),
                parent_task_id: metadata_string(incoming_meta, &["parent_task_id"]),
                parent_execution_id: metadata_string(
                    incoming_meta,
                    &["parent_execution_id", "parent_exec_id"],
                ),
                trace_id: metadata_string(incoming_meta, &["trace_id"]),
                span_id: metadata_string(incoming_meta, &["span_id"]),
                parent_span_id: metadata_string(incoming_meta, &["parent_span_id"]),
                correlation_id: metadata_string(incoming_meta, &["correlation_id", "dispatch_id"]),
                dispatch_id: metadata_string(incoming_meta, &["dispatch_id", "correlation_id"]),
                child_agent_id: metadata_string(incoming_meta, &["child_agent_id"]),
                specialist_role: metadata_string(
                    incoming_meta,
                    &["specialist_role", "child_agent_id"],
                ),
                artifact_ref: metadata_string(incoming_meta, &["artifact_ref"]),
            })
            .await?;

        state.status = AgentStatus::Working;
        state.current_execution = Some(execution.execution_id.clone());

        info!(
            agent_id = %state.agent_id,
            execution_id = %execution.execution_id,
            sender = %message.sender,
            message_type = %message.msg_type,
            "Processing steering message"
        );

        let timestamp = Utc::now().to_rfc3339();
        self.ams
            .emit_execution_chunk(
                &fleet_execution_id,
                &ExecutionChunkRequest {
                    agent_id: &state.agent_id,
                    tenant_id: "default",
                    execution_id: &fleet_execution_id,
                    chunk_type: "start",
                    timestamp: &timestamp,
                    data: ExecutionChunkData {
                        model: Some(requested_model.as_str()),
                        ..Default::default()
                    },
                },
            )
            .await?;

        let started_at = std::time::Instant::now();
        let system_prompt = self
            .hand
            .as_ref()
            .and_then(|hand| hand.system_prompt.as_deref());

        // Tools are enabled either by archetype (team-leads and orchestrators
        // always get the dispatch/wait/synthesize loop) or by AMS birth grants
        // (enable_tools=true opts any agent into the tool loop).
        let archetype = self
            .hand
            .as_ref()
            .map(|h| h.manifest.hand.archetype.as_str())
            .unwrap_or("");
        let archetype_enables_tools = matches!(archetype, "team-lead" | "orchestrator");
        let grants_enable_tools = self
            .grants
            .as_ref()
            .map(|g| g.enable_tools)
            .unwrap_or(false);
        let env_enable_tools = std::env::var("AUTOMATON_ENABLE_TOOLS")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let has_tools = env_enable_tools || archetype_enables_tools || grants_enable_tools;

        // If this activation was spawned by a parent orchestrator's
        // dispatch_to_tl, capture the rollup breadcrumbs so the tool loop
        // can ding the parent when it finishes. For msg_type == "rollup"
        // arriving at an orchestrator, we're the terminus (rollup_target
        // stays None) and the prompt is augmented instead so the LLM
        // knows to synthesize the TL's result for the user.
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
                state,
                &fleet_execution_id,
                &prompt,
                &requested_model,
                system_prompt,
                started_at,
                rollup_target,
                chat_session_id,
            )
            .await
        } else {
            // Non-tooled agents: single-shot response (original behavior)
            self.run_single_shot(
                state,
                &fleet_execution_id,
                &prompt,
                &requested_model,
                started_at,
            )
            .await
        }?;

        let timestamp = Utc::now().to_rfc3339();
        self.ams
            .emit_execution_chunk(
                &fleet_execution_id,
                &ExecutionChunkRequest {
                    agent_id: &state.agent_id,
                    tenant_id: "default",
                    execution_id: &fleet_execution_id,
                    chunk_type: final_event.chunk_type,
                    timestamp: &timestamp,
                    data: ExecutionChunkData {
                        content: final_event.content.as_deref(),
                        tokens_in: final_event.tokens_in,
                        tokens_out: final_event.tokens_out,
                        duration_ms: final_event.duration_ms,
                        error: final_event.error.as_deref(),
                        model: final_event.model.as_deref(),
                        ..Default::default()
                    },
                },
            )
            .await?;

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
    ) -> Result<ExecutionCompleteEvent> {
        let result = self.generate_response(prompt).await;
        match result {
            Ok(result) => {
                state.token_count = state
                    .token_count
                    .saturating_add(result.input_tokens + result.output_tokens);
                state.context_pct = ((state.token_count as f64 / state.max_tokens as f64) * 100.0)
                    .clamp(0.0, 100.0);

                let timestamp = Utc::now().to_rfc3339();
                self.ams
                    .emit_execution_chunk(
                        fleet_execution_id,
                        &ExecutionChunkRequest {
                            agent_id: &state.agent_id,
                            tenant_id: "default",
                            execution_id: fleet_execution_id,
                            chunk_type: "output",
                            timestamp: &timestamp,
                            data: ExecutionChunkData {
                                content: Some(result.content.as_str()),
                                model: Some(result.model.as_str()),
                                ..Default::default()
                            },
                        },
                    )
                    .await?;

                info!(
                    agent_id = %state.agent_id,
                    provider = %result.provider,
                    model = %result.model,
                    "Steering message completed (single-shot)"
                );

                Ok(ExecutionCompleteEvent {
                    tokens_in: Some(result.input_tokens),
                    tokens_out: Some(result.output_tokens),
                    duration_ms: Some(started_at.elapsed().as_millis() as u64),
                    model: Some(result.model),
                    chunk_type: "complete",
                    ..Default::default()
                })
            }
            Err(error) => {
                warn!(error = %error, "Steering message failed (single-shot)");
                Ok(ExecutionCompleteEvent {
                    duration_ms: Some(started_at.elapsed().as_millis() as u64),
                    error: Some(error.to_string()),
                    model: Some(requested_model.to_string()),
                    chunk_type: "error",
                    ..Default::default()
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
    #[allow(clippy::too_many_arguments)]
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
    ) -> Result<ExecutionCompleteEvent> {
        let archetype = self
            .hand
            .as_ref()
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
            "orchestrator" => Self::orchestrator_tool_definitions(),
            _ => Self::mcp_bridge_tool_definitions(),
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

            let response = self
                .ams
                .complete_with_tools(&ToolCompletionRequest {
                    messages: &messages,
                    tools: &tools,
                    max_tokens: 4000,
                    model: Some(requested_model),
                    temperature: Some(0.3),
                })
                .await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    warn!(error = %e, iteration = iteration, "Tool loop LLM call failed");
                    return Ok(ExecutionCompleteEvent {
                        duration_ms: Some(started_at.elapsed().as_millis() as u64),
                        error: Some(e.to_string()),
                        model: Some(requested_model.to_string()),
                        chunk_type: "error",
                        ..Default::default()
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
                let func_args_str = func
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                let func_args: serde_json::Value =
                    serde_json::from_str(func_args_str).unwrap_or_default();

                info!(
                    agent_id = %state.agent_id,
                    tool = func_name,
                    args = %func_args,
                    "Executing tool call"
                );

                // Emit tool use telemetry
                let timestamp = Utc::now().to_rfc3339();
                let _ = self
                    .ams
                    .emit_execution_chunk(
                        fleet_execution_id,
                        &ExecutionChunkRequest {
                            agent_id: &state.agent_id,
                            tenant_id: "default",
                            execution_id: fleet_execution_id,
                            chunk_type: "tool_use",
                            timestamp: &timestamp,
                            data: ExecutionChunkData {
                                tool_name: Some(func_name),
                                tool_input: Some(&func_args),
                                ..Default::default()
                            },
                        },
                    )
                    .await;

                let caller_registry_execution_id = state
                    .current_execution
                    .as_deref()
                    .unwrap_or(fleet_execution_id);
                let tool_result = self
                    .execute_tool(
                        func_name,
                        &func_args,
                        &state.agent_id,
                        Some(caller_registry_execution_id),
                        chat_session_id,
                    )
                    .await;

                info!(
                    agent_id = %state.agent_id,
                    tool = func_name,
                    result_len = tool_result.len(),
                    "Tool call completed"
                );

                // Emit tool result telemetry
                let timestamp = Utc::now().to_rfc3339();
                let _ = self
                    .ams
                    .emit_execution_chunk(
                        fleet_execution_id,
                        &ExecutionChunkRequest {
                            agent_id: &state.agent_id,
                            tenant_id: "default",
                            execution_id: fleet_execution_id,
                            chunk_type: "tool_result",
                            timestamp: &timestamp,
                            data: ExecutionChunkData {
                                tool_name: Some(func_name),
                                tool_output: Some(tool_result.as_str()),
                                ..Default::default()
                            },
                        },
                    )
                    .await;

                // Add tool result message
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tc_id,
                    "content": tool_result,
                }));
            }
        }

        state.token_count = state
            .token_count
            .saturating_add(total_in_tokens + total_out_tokens);
        state.context_pct =
            ((state.token_count as f64 / state.max_tokens as f64) * 100.0).clamp(0.0, 100.0);

        // Emit final output
        if !final_text.is_empty() {
            let timestamp = Utc::now().to_rfc3339();
            let _ = self
                .ams
                .emit_execution_chunk(
                    fleet_execution_id,
                    &ExecutionChunkRequest {
                        agent_id: &state.agent_id,
                        tenant_id: "default",
                        execution_id: fleet_execution_id,
                        chunk_type: "output",
                        timestamp: &timestamp,
                        data: ExecutionChunkData {
                            content: Some(final_text.as_str()),
                            model: Some(requested_model),
                            ..Default::default()
                        },
                    },
                )
                .await;

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
                let summary = summarize_rollup_text(&final_text);
                let child_registry_execution_id = state
                    .current_execution
                    .as_deref()
                    .unwrap_or(fleet_execution_id);
                let mut rollup_meta = serde_json::json!({
                    "parent_exec_id": parent_exec,
                    "parent_execution_id": parent_exec,
                    "child_exec_id": child_registry_execution_id,
                    "child_fleet_execution_id": fleet_execution_id,
                    "child_agent_id": state.agent_id,
                    "memory_id": memory_id.clone().unwrap_or_default(),
                });
                // Thread the dashboard chat session all the way back up so
                // the parent's rollup-triggered synthesis turn can post
                // its reply into the same conversation.
                if let Some(cs) = chat_session_id {
                    rollup_meta["chat_session_id"] = serde_json::Value::String(cs.to_string());
                }
                if let Err(e) = self
                    .ams
                    .send_steering_message(
                        parent_agent,
                        &summary,
                        "rollup",
                        &state.agent_id,
                        Some(&rollup_meta),
                    )
                    .await
                {
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
        if let Some(cs) = chat_session_id
            && !final_text.trim().is_empty()
        {
            if let Err(e) = self
                .ams
                .post_chat_message(cs, &final_text, &state.agent_id, Some(requested_model))
                .await
            {
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

        Ok(ExecutionCompleteEvent {
            tokens_in: Some(total_in_tokens),
            tokens_out: Some(total_out_tokens),
            duration_ms: Some(started_at.elapsed().as_millis() as u64),
            model: Some(requested_model.to_string()),
            chunk_type: "complete",
            ..Default::default()
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
                let worker = args
                    .get("worker_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let timeout_secs = args
                    .get("timeout_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(900);
                if worker.is_empty() || task.is_empty() {
                    return serde_json::json!({"error": "worker_name and task are required"})
                        .to_string();
                }

                // Step 1: dispatch with durable lineage. AMS returns an
                // execution_id when spawn_triggered=true, but a worker that
                // was ALREADY ALIVE only gets a queued Warden message; its
                // execution row shows up later, when it polls and registers
                // the task. correlation_id/dispatch_id is what lets us find
                // that later Observatory row instead of dead-ending on
                // "enqueued_only".
                let dispatch_id = format!("dispatch-{}", Uuid::new_v4().simple());
                let mut dispatch_meta = serde_json::json!({
                    "parent_agent_id": caller_agent_id,
                    "correlation_id": dispatch_id,
                    "dispatch_id": dispatch_id,
                    "child_agent_id": worker,
                    "specialist_role": worker,
                });
                if let Some(exec) = caller_exec_id {
                    dispatch_meta["parent_exec_id"] = serde_json::Value::String(exec.to_string());
                    dispatch_meta["parent_execution_id"] =
                        serde_json::Value::String(exec.to_string());
                }
                if let Some(cs) = chat_session_id {
                    dispatch_meta["chat_session_id"] = serde_json::Value::String(cs.to_string());
                }

                let dispatch = match self
                    .ams
                    .send_steering_message(
                        worker,
                        task,
                        "task",
                        caller_agent_id,
                        Some(&dispatch_meta),
                    )
                    .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        return serde_json::json!({
                            "ok": false, "error": format!("dispatch: {}", e),
                        })
                        .to_string();
                    }
                };

                let mut exec_id = execution_id_from_value(&dispatch);

                // Step 2: poll the observatory until the child row appears
                // and reaches a terminal status, then return its output so
                // the caller can synthesize a real fan-in rollup.
                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
                let poll_interval = std::time::Duration::from_secs(2);
                let parent_exec_owned = caller_exec_id.map(str::to_string);

                let mut last_exec: Option<serde_json::Value> = None;
                let mut last_status = "unknown".to_string();

                loop {
                    if std::time::Instant::now() > deadline {
                        // Running out of wait is NOT a failed dispatch: the
                        // worker keeps going and its output lands on the same
                        // execution row. Hand back the exact poll call that
                        // fans it in on a later turn.
                        return match exec_id.as_deref() {
                            Some(id) => {
                                let mut pending = pending_worker_result(
                                    Some(worker),
                                    id,
                                    &last_status,
                                    timeout_secs,
                                    last_exec.as_ref(),
                                );
                                pending["correlation_id"] =
                                    serde_json::Value::String(dispatch_id.clone());
                                pending.to_string()
                            }
                            None => serde_json::json!({
                                "ok": false,
                                "dispatched_to": worker,
                                "execution_id": serde_json::Value::Null,
                                "status": "no_execution_row",
                                "terminal": false,
                                "waited_secs": timeout_secs,
                                "correlation_id": dispatch_id,
                                "note": concat!(
                                    "The worker was dispatched but no execution row ",
                                    "appeared for this correlation_id within the wait ",
                                    "window. The steering message is queued; confirm the ",
                                    "worker is alive before re-dispatching.",
                                ),
                                "response": dispatch,
                            })
                            .to_string(),
                        };
                    }

                    // The row may not exist yet (already-alive worker), so
                    // look it up by the lineage we stamped on the dispatch.
                    if exec_id.is_none() {
                        match self
                            .ams
                            .find_execution_by_lineage(
                                Some(&dispatch_id),
                                parent_exec_owned.as_deref(),
                                Some(worker),
                            )
                            .await
                        {
                            Ok(Some(found)) => {
                                exec_id = execution_id_from_value(&found);
                                if exec_id.is_none() {
                                    tracing::debug!(
                                        correlation_id = %dispatch_id,
                                        "lineage lookup returned row without execution id"
                                    );
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                tracing::debug!(
                                    correlation_id = %dispatch_id,
                                    err = %e,
                                    "lineage lookup transient"
                                );
                            }
                        }
                    }

                    if let Some(current_exec_id) = exec_id.as_deref() {
                        match self.ams.get_execution(current_exec_id).await {
                            Ok(exec) => {
                                let status = exec
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                if is_terminal_exec_status(&status) {
                                    let mut terminal = terminal_worker_result(
                                        Some(worker),
                                        current_exec_id,
                                        &status,
                                        &exec,
                                    );
                                    terminal["correlation_id"] =
                                        serde_json::Value::String(dispatch_id.clone());
                                    return terminal.to_string();
                                }
                                last_status = status;
                                last_exec = Some(exec);
                            }
                            Err(e) => {
                                // 404s are expected for the first few polls
                                // while the background spawn writes the row.
                                tracing::debug!(
                                    exec_id = %current_exec_id,
                                    err = %e,
                                    "get_execution transient"
                                );
                            }
                        }
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            "poll_worker_execution" => {
                // Fan in on a worker execution dispatched on an earlier
                // turn. Without this, "re-check on your next turn" (which
                // the TL system prompts promise) is impossible: the only
                // dispatch tool starts new work, so a timed-out wait meant
                // re-running a task that was already in flight.
                let exec_id = args
                    .get("execution_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if exec_id.is_empty() {
                    return serde_json::json!({"error": "execution_id is required"}).to_string();
                }
                let wait_secs = args
                    .get("wait_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    .min(MAX_POLL_WAIT_SECS);

                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_secs(wait_secs);
                let poll_interval = std::time::Duration::from_secs(2);
                let mut last_exec: Option<serde_json::Value> = None;
                let mut last_status = "unknown".to_string();
                let mut last_error: Option<String> = None;

                loop {
                    match self.ams.get_execution(exec_id).await {
                        Ok(exec) => {
                            let status = exec
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            if is_terminal_exec_status(&status) {
                                return terminal_worker_result(None, exec_id, &status, &exec)
                                    .to_string();
                            }
                            last_status = status;
                            last_exec = Some(exec);
                        }
                        Err(e) => {
                            last_error = Some(e.to_string());
                            tracing::debug!(exec_id = %exec_id, err = %e, "poll_worker_execution transient");
                        }
                    }

                    if std::time::Instant::now() >= deadline {
                        // Never seen at all: say so rather than implying
                        // work is in flight that may not exist.
                        if last_exec.is_none() {
                            return serde_json::json!({
                                "ok": false,
                                "execution_id": exec_id,
                                "status": "unknown",
                                "terminal": false,
                                "error": last_error.unwrap_or_else(|| {
                                    "execution row not found".to_string()
                                }),
                                "note": concat!(
                                    "No observatory row for this execution id. ",
                                    "Check the id before assuming the work is ",
                                    "still running.",
                                ),
                            })
                            .to_string();
                        }
                        return pending_worker_result(
                            None,
                            exec_id,
                            &last_status,
                            wait_secs,
                            last_exec.as_ref(),
                        )
                        .to_string();
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
            "dispatch_to_tl" => {
                let tl_name = args.get("tl_name").and_then(|v| v.as_str()).unwrap_or("");
                let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or("normal");
                if tl_name.is_empty() || task.is_empty() {
                    return serde_json::json!({"error": "tl_name and task are required"})
                        .to_string();
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
                match self
                    .ams
                    .send_steering_message(
                        tl_name,
                        task,
                        "task",
                        caller_agent_id,
                        rollup_meta.as_ref(),
                    )
                    .await
                {
                    Ok(resp) => serde_json::json!({
                        "ok": true,
                        "dispatched_to": tl_name,
                        "priority": priority,
                        "response": resp,
                    })
                    .to_string(),
                    Err(e) => serde_json::json!({
                        "ok": false,
                        "error": e.to_string(),
                    })
                    .to_string(),
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
                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or("normal");
                if title.is_empty() {
                    return serde_json::json!({"error": "title is required"}).to_string();
                }
                match self
                    .ams
                    .create_goal_task(title, description, priority, caller_agent_id)
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "create_memory" => {
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                if content.trim().is_empty() {
                    return serde_json::json!({"error": "content is required"}).to_string();
                }
                let tier = args
                    .get("tier")
                    .and_then(|v| v.as_str())
                    .unwrap_or("episodic");
                if !matches!(tier, "episodic" | "semantic" | "procedural") {
                    return serde_json::json!({
                        "error": "tier must be one of episodic, semantic, procedural"
                    })
                    .to_string();
                }
                let mut tags: Vec<String> = args
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let agent_tag = format!("agent:{}", caller_agent_id);
                if !tags.iter().any(|tag| tag == &agent_tag) {
                    tags.push(agent_tag);
                }
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{} memory receipt", caller_agent_id));

                let memory = abot_ams::memory::CreateMemoryRequest {
                    title,
                    content: content.to_string(),
                    memory_tier: tier.to_string(),
                    entity_type: "event".to_string(),
                    importance: 0.6,
                    tags,
                    metadata: Some(serde_json::json!({
                        "source_agent": caller_agent_id,
                        "source": "agent_tool:create_memory",
                    })),
                };
                match self.ams.create_memory(memory).await {
                    Ok(resp) => serde_json::json!({
                        "ok": true,
                        "memory_id": resp.get("id").or_else(|| resp.get("memory_id")),
                        "response": resp,
                    })
                    .to_string(),
                    Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
                }
            }
            "search_memories" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                match self.ams.search_memories(query, limit).await {
                    Ok(results) => {
                        let summaries: Vec<serde_json::Value> = results
                            .iter()
                            .map(|r| {
                                let memory =
                                    r.get("memory").cloned().unwrap_or(serde_json::Value::Null);
                                let file_path = memory
                                    .get("file_path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let tags = memory
                                    .get("tags")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Array(vec![]));
                                let snippet = r
                                    .get("content_snippet")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let score = r
                                    .get("relevance_score")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                serde_json::json!({
                                    "file_path": file_path,
                                    "tags": tags,
                                    "snippet": truncate_chars(&snippet, 200),
                                    "score": score,
                                })
                            })
                            .collect();
                        serde_json::json!({"results": summaries}).to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "complete_task" => {
                // Board closure. Until this existed a TL could only *say*
                // a CAP task was done; the tasks row stayed CLAIMED until
                // the 4h claim TTL expired and the next standup re-claimed
                // and re-ran it (bug d1c8be7c).
                let task_id = args
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();
                let summary = args
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();
                if task_id.is_empty() || summary.is_empty() {
                    return serde_json::json!({"error": "task_id and summary are required"})
                        .to_string();
                }
                if Uuid::parse_str(task_id).is_err() {
                    return serde_json::json!({
                        "error": "task_id must be the CAP task UUID from the [BOARD] line"
                    })
                    .to_string();
                }
                let result = complete_task_result(
                    summary,
                    caller_agent_id,
                    caller_exec_id,
                    args.get("worker_execution_ids"),
                );
                match self.ams.complete_task(task_id, &result).await {
                    Ok(resp) => resp.to_string(),
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "get_task_board" => match self.ams.get_task_board().await {
                Ok(board) => task_board_summary(&board).to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            },
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
                        let names: Vec<String> = agents
                            .iter()
                            .filter_map(|a| {
                                let id = a.get("agent_id").and_then(|v| v.as_str())?;
                                if id.starts_with("tl-") || id == "memory-curator" {
                                    None
                                } else {
                                    Some(id.to_string())
                                }
                            })
                            .collect();
                        serde_json::json!({
                            "workers": names,
                            "count": names.len(),
                            "scope_prefix": prefix,
                        })
                        .to_string()
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "mcp_list_servers" => match self.ams.mcp_list_servers().await {
                Ok(resp) => resp.to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            },
            "mcp_call_tool" => {
                let server = args.get("server").and_then(|v| v.as_str()).unwrap_or("");
                let tool = args.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let timeout_seconds = args
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30) as f64;
                if server.is_empty() || tool.is_empty() {
                    return serde_json::json!({"error": "server and tool are required"})
                        .to_string();
                }
                match self
                    .ams
                    .mcp_call_tool(server, tool, &tool_args, timeout_seconds)
                    .await
                {
                    Ok(resp) => resp.to_string(),
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            _ => serde_json::json!({"error": format!("Unknown tool: {}", name)}).to_string(),
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
    fn create_memory_tool_definition() -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "create_memory",
                "description": "Write a durable AMS memory receipt or domain note.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "Memory content to persist."
                        },
                        "tier": {
                            "type": "string",
                            "enum": ["episodic", "semantic", "procedural"],
                            "description": "Memory tier (default episodic).",
                            "default": "episodic"
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Tags to attach to the memory.",
                            "default": []
                        },
                        "title": {
                            "type": "string",
                            "description": "Optional short title."
                        }
                    },
                    "required": ["content"]
                }
            }
        })
    }

    fn get_task_board_tool_definition() -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_task_board",
                "description": "Read the AMS task board summary: pending/active/done/failed counts and top task titles.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            }
        })
    }

    fn complete_task_tool_definition() -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "complete_task",
                "description": "Close out the CAP board task this dispatch claimed for you (the task_id in its [BOARD] line). Call it exactly once, only after the work is actually finished and every worker result has been fanned in. Never call it while a worker is still_running or the task is blocked; the board stays CLAIMED until you do, so a finished task you never close will be re-dispatched tomorrow.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "CAP task UUID from the [BOARD] line of the dispatch."
                        },
                        "summary": {
                            "type": "string",
                            "description": "What was delivered, with receipts: commits, files, memory ids, and the worker execution ids that produced the result."
                        },
                        "worker_execution_ids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Execution ids (spawn-*) of the workers whose output this completion rests on."
                        }
                    },
                    "required": ["task_id", "summary"]
                }
            }
        })
    }

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
                    let name = a
                        .get("agent_id")
                        .or_else(|| a.get("name"))
                        .and_then(|v| v.as_str())?;
                    let desc = a
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    if desc.is_empty() {
                        Some(format!("- {}", name))
                    } else {
                        let truncated = truncate_chars(desc, 160);
                        let short = if truncated.len() < desc.len() {
                            format!("{}...", truncated)
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
                            },
                            "timeout_secs": {
                                "type": "integer",
                                "description": "How long to wait for the worker's result before returning status 'still_running' (default 900). Workers keep running past this and their output persists on the Observatory execution row, so running out of time is not a failure: fan the result in later with poll_worker_execution.",
                                "default": 900
                            }
                        },
                        "required": ["worker_name", "task"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "poll_worker_execution",
                    "description": "Fan in on a worker execution you dispatched earlier (including on a previous turn) by its execution_id. Returns the terminal status and full output once the worker finishes, or status 'still_running' with partial output. Use this instead of re-dispatching a task whose wait window ran out.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "execution_id": {
                                "type": "string",
                                "description": "The execution_id returned by dispatch_to_worker (e.g. spawn-ab12cd34ef56)."
                            },
                            "wait_secs": {
                                "type": "integer",
                                "description": "Optionally keep polling up to this many seconds (max 600) before giving up. Default 0 = check once and return.",
                                "default": 0
                            }
                        },
                        "required": ["execution_id"]
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
            Self::create_memory_tool_definition(),
            Self::get_task_board_tool_definition(),
            Self::complete_task_tool_definition(),
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
            Self::create_memory_tool_definition(),
            Self::get_task_board_tool_definition(),
        ]
    }

    /// Tool definitions for non-TL agents when tools are enabled via grants/env.
    ///
    /// These tools bridge into AMS MCP Gateway so a worker can actually execute
    /// MCP-backed capabilities instead of being stuck with orchestrator-only tools.
    fn mcp_bridge_tool_definitions() -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "mcp_list_servers",
                    "description": "List MCP servers available through AMS gateway and their health/connectivity stats.",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "mcp_call_tool",
                    "description": "Call a tool on an MCP server through AMS MCP gateway.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "server": {
                                "type": "string",
                                "description": "MCP server name (use mcp_list_servers first)."
                            },
                            "tool": {
                                "type": "string",
                                "description": "Tool name exposed by the selected MCP server."
                            },
                            "arguments": {
                                "type": "object",
                                "description": "Arguments object passed to the MCP tool.",
                                "default": {}
                            },
                            "timeout_seconds": {
                                "type": "integer",
                                "description": "Optional timeout in seconds (default 30).",
                                "default": 30
                            }
                        },
                        "required": ["server", "tool"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "search_memories",
                    "description": "Search AMS memories for relevant context before/after MCP calls.",
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
            Self::create_memory_tool_definition(),
            Self::get_task_board_tool_definition(),
        ]
    }

    async fn generate_response(&self, prompt: &str) -> Result<GenerationResult> {
        let system_prompt = self
            .hand
            .as_ref()
            .and_then(|hand| hand.system_prompt.as_deref());

        if let Some(bridge) = self.kilo_bridge() {
            let mode = self.kilo_mode();
            let prompt_for_kilo = if let Some(system) = &system_prompt {
                format!("{system}\n\nUser request:\n{prompt}")
            } else {
                prompt.to_string()
            };

            let response =
                tokio::task::spawn_blocking(move || bridge.execute(&prompt_for_kilo, mode))
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
        let response = self
            .ams
            .complete(&CompletionRequest {
                prompt,
                max_tokens: 4000,
                role: "agent",
                model: Some(&requested_model),
                system_prompt,
                temperature: None,
            })
            .await?;

        Ok(GenerationResult {
            content: response.text,
            model: response.model,
            provider: response.provider,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    fn requested_model(&self) -> String {
        if let Some(hand) = &self.hand
            && !hand.manifest.hand.default_model.trim().is_empty()
        {
            return hand.manifest.hand.default_model.trim().to_string();
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
        let _death_response = self
            .ams
            .death(abot_ams::warden::DeathRequest {
                agent_id: state.agent_id.clone(),
                original_goal: String::new(), // TODO: track current goal
                next_action: String::new(),   // TODO: determine next action
                completed_subtasks: vec![],   // TODO: track subtasks
                remaining_subtasks: vec![],
                handoff_notes: None,
                memories: vec![], // TODO: crystallize session memories
                context_pct: state.context_pct,
            })
            .await?;

        info!("Death ritual complete. Goodbye.");
        Ok(())
    }
}

/// Result payload stored on the CAP task row when a TL closes it.
fn complete_task_result(
    summary: &str,
    completed_by: &str,
    execution_id: Option<&str>,
    worker_execution_ids: Option<&serde_json::Value>,
) -> serde_json::Value {
    let workers: Vec<String> = worker_execution_ids
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    serde_json::json!({
        "summary": summary,
        "completed_by": completed_by,
        "execution_id": execution_id,
        "worker_execution_ids": workers,
        "completed_at": Utc::now().to_rfc3339(),
        "source": "abot.complete_task",
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Runtime, complete_task_result, is_terminal_exec_status, pending_worker_result,
        summarize_rollup_text, terminal_worker_result, truncate_chars,
    };

    #[test]
    fn truncate_chars_respects_utf8_boundaries() {
        let value = format!("{}étail", "a".repeat(199));
        let truncated = truncate_chars(&value, 200);

        assert_eq!(truncated.chars().count(), 200);
        assert_eq!(truncated, format!("{}é", "a".repeat(199)));
    }

    #[test]
    fn truncate_chars_supports_description_ellipsis() {
        let description = format!("{}émore", "a".repeat(159));
        let truncated = truncate_chars(&description, 160);
        let short = if truncated.len() < description.len() {
            format!("{}...", truncated)
        } else {
            description.clone()
        };

        assert_eq!(short, format!("{}é...", "a".repeat(159)));
    }

    #[test]
    fn summarize_rollup_text_appends_ellipsis_after_500_chars() {
        let value = format!("{}étail", "a".repeat(499));

        assert_eq!(
            summarize_rollup_text(&value),
            format!("{}é...", "a".repeat(499))
        );
    }

    #[test]
    fn summarize_rollup_text_keeps_short_values_unchanged() {
        let value = "short summary";

        assert_eq!(summarize_rollup_text(value), value);
    }

    #[test]
    fn error_and_timeout_rows_are_terminal() {
        // AMS writes these two on the worker side; treating them as
        // non-terminal made the poller wait out its whole deadline on a
        // row that was never going to change again.
        for status in ["completed", "failed", "killed", "error", "timeout"] {
            assert!(
                is_terminal_exec_status(status),
                "{status} should be terminal"
            );
        }
        for status in ["queued", "running", "bridged", "unknown"] {
            assert!(
                !is_terminal_exec_status(status),
                "{status} should not be terminal"
            );
        }
    }

    #[test]
    fn terminal_result_is_ok_only_when_completed() {
        let exec = serde_json::json!({"output": "done", "duration_ms": 1200});
        let completed = terminal_worker_result(Some("coder"), "spawn-1", "completed", &exec);
        assert_eq!(completed["ok"], true);
        assert_eq!(completed["output"], "done");
        assert_eq!(completed["terminal"], true);

        let failed = terminal_worker_result(Some("coder"), "spawn-1", "error", &exec);
        assert_eq!(failed["ok"], false);
        assert_eq!(failed["status"], "error");
        assert_eq!(failed["terminal"], true);
    }

    #[test]
    fn pending_result_carries_partial_output_and_resume_call() {
        let exec = serde_json::json!({"status": "running", "output": "half a report"});
        let pending = pending_worker_result(Some("coder"), "spawn-9", "running", 900, Some(&exec));

        assert_eq!(pending["status"], "still_running");
        assert_eq!(pending["terminal"], false);
        assert_eq!(pending["observed_status"], "running");
        assert_eq!(pending["partial_output"], "half a report");
        assert_eq!(pending["resume_with"]["tool"], "poll_worker_execution");
        assert_eq!(
            pending["resume_with"]["arguments"]["execution_id"],
            "spawn-9"
        );
    }

    #[test]
    fn pending_result_without_a_row_has_no_partial_output() {
        let pending = pending_worker_result(None, "spawn-9", "unknown", 180, None);

        assert_eq!(pending["status"], "still_running");
        assert!(pending["partial_output"].is_null());
        assert_eq!(
            pending["resume_with"]["arguments"]["execution_id"],
            "spawn-9"
        );
    }
    #[test]
    fn tl_toolset_exposes_complete_task() {
        let names: Vec<String> = Runtime::tl_tool_definitions(&[])
            .iter()
            .filter_map(|t| t["function"]["name"].as_str().map(str::to_string))
            .collect();

        assert!(names.iter().any(|n| n == "complete_task"));
        assert!(names.iter().any(|n| n == "poll_worker_execution"));
    }

    #[test]
    fn complete_task_result_carries_receipts() {
        let workers = serde_json::json!(["spawn-1", 7, "spawn-2"]);
        let result = complete_task_result("done", "tl-engineering", Some("exec-9"), Some(&workers));

        assert_eq!(result["summary"], "done");
        assert_eq!(result["completed_by"], "tl-engineering");
        assert_eq!(result["execution_id"], "exec-9");
        assert_eq!(
            result["worker_execution_ids"],
            serde_json::json!(["spawn-1", "spawn-2"])
        );
        assert_eq!(result["source"], "abot.complete_task");
    }
}
