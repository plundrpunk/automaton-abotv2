use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

use abot_ams::client::AmsClient;
use abot_ams::warden::{Directive, HeartbeatPayload};

/// Runtime state snapshot sent with each heartbeat.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeState {
    pub agent_id: String,
    pub context_pct: f64,
    pub status: String,
    pub current_execution: Option<String>,
}

/// Sends periodic heartbeats to AMS with agent runtime state.
///
/// The reporter is a thin wrapper — it formats the RuntimeState
/// into a HeartbeatPayload and calls AmsClient.heartbeat().
/// All lifecycle decisions come back as a Directive from AMS.
pub struct HeartbeatReporter {
    ams_client: AmsClient,
    interval_secs: u64,
}

impl HeartbeatReporter {
    /// Create a new heartbeat reporter.
    pub fn new(ams_client: AmsClient, interval_secs: u64) -> Self {
        Self {
            ams_client,
            interval_secs,
        }
    }

    /// Send a heartbeat with current runtime state and return AMS directive.
    ///
    /// AMS evaluates context_pct against governance thresholds and returns
    /// one of: Continue, Warn, BeginDeathRitual, HardStop.
    pub async fn tick(&self, state: &RuntimeState) -> Result<Directive> {
        debug!(
            agent_id = %state.agent_id,
            context_pct = state.context_pct,
            status = %state.status,
            "Sending heartbeat to AMS"
        );

        let payload = HeartbeatPayload {
            agent_id: state.agent_id.clone(),
            status: state.status.clone(),
            context_pct: state.context_pct,
            execution_id: state.current_execution.clone(),
            metadata: None,
        };

        let response = self.ams_client.heartbeat(payload).await?;

        debug!(
            directive = ?response.directive,
            message = ?response.message,
            "Received directive from AMS"
        );

        Ok(response.directive)
    }

    /// Get the configured interval in seconds.
    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_state_serialization() {
        let state = RuntimeState {
            agent_id: "test-agent".to_string(),
            context_pct: 75.5,
            status: "running".to_string(),
            current_execution: Some("task-123".to_string()),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: RuntimeState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_id, "test-agent");
        assert_eq!(deserialized.context_pct, 75.5);
    }
}
