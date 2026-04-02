use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::config::AbotConfig;
use crate::hand::{LoadedHand, load_hand};
use abot_ams::client::{AmsClient, AmsConfig, SteeringMessage};
use abot_ams::fleet::{ExecutionChunkData, ExecutionChunkRequest, RegisterExecutionRequest};
use abot_ams::llm::CompletionRequest;
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

        let result = self.generate_response(&prompt).await;
        let final_event = match result {
            Ok(result) => {
                state.token_count = state
                    .token_count
                    .saturating_add(result.input_tokens + result.output_tokens);
                state.context_pct = ((state.token_count as f64 / state.max_tokens as f64) * 100.0)
                    .clamp(0.0, 100.0);

                self.ams.emit_execution_chunk(
                    &fleet_execution_id,
                    &ExecutionChunkRequest {
                        agent_id: state.agent_id.clone(),
                        tenant_id: "default".to_string(),
                        execution_id: fleet_execution_id.clone(),
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
                    execution_id = %execution.execution_id,
                    provider = %result.provider,
                    model = %result.model,
                    "Steering message completed"
                );

                ExecutionChunkRequest {
                    agent_id: state.agent_id.clone(),
                    tenant_id: "default".to_string(),
                    execution_id: fleet_execution_id.clone(),
                    chunk_type: "complete".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    data: ExecutionChunkData {
                        tokens_in: Some(result.input_tokens),
                        tokens_out: Some(result.output_tokens),
                        duration_ms: Some(started_at.elapsed().as_millis() as u64),
                        model: Some(result.model),
                        ..Default::default()
                    },
                }
            }
            Err(error) => {
                warn!(
                    agent_id = %state.agent_id,
                    execution_id = %execution.execution_id,
                    error = %error,
                    "Steering message failed"
                );
                ExecutionChunkRequest {
                    agent_id: state.agent_id.clone(),
                    tenant_id: "default".to_string(),
                    execution_id: fleet_execution_id.clone(),
                    chunk_type: "error".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    data: ExecutionChunkData {
                        duration_ms: Some(started_at.elapsed().as_millis() as u64),
                        error: Some(error.to_string()),
                        model: Some(requested_model),
                        ..Default::default()
                    },
                }
            }
        };

        self.ams.emit_execution_chunk(&fleet_execution_id, &final_event).await?;
        state.status = AgentStatus::Idle;
        state.current_execution = None;
        Ok(())
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
