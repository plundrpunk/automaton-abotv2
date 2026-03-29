use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

use crate::config::AbotConfig;
use crate::hand::{LoadedHand, load_hand};
use abot_ams::client::{AmsClient, AmsConfig};
use abot_ams::warden::{BirthRequest, Directive};
use abot_telemetry::heartbeat::{HeartbeatReporter, RuntimeState as TelemetryState};

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

                // Shutdown signal (SIGTERM, SIGINT)
                _ = self.shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    state.status = AgentStatus::Dying;
                    self.execute_death_ritual(&state).await?;
                    return Ok(());
                },

                // TODO: Channel message handler
                // TODO: Task execution handler
                // TODO: MCP request handler
            }
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
