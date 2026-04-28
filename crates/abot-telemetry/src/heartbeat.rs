use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use abot_ams::client::AmsClient;
use abot_ams::fleet::{FleetHeartbeatMetrics, FleetHeartbeatRequest, FleetHeartbeatUsage};
use abot_ams::warden::{Directive, HeartbeatPayload};

use crate::metrics::SystemMetrics;

/// Runtime state snapshot sent with each heartbeat.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeState {
    pub agent_id: String,
    pub context_pct: f64,
    pub status: String,
    pub current_execution: Option<String>,
}

/// Counters accumulated between heartbeats and flushed on every tick().
///
/// Clone + Arc lets the runtime hand this to any tool-call / token-accounting
/// path so it can bump counters without holding a mutex.
#[derive(Clone, Default)]
pub struct UsageCounters {
    tokens_in: Arc<AtomicU64>,
    tokens_out: Arc<AtomicU64>,
    executions: Arc<AtomicU64>,
}

impl UsageCounters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_tokens_in(&self, n: u64) {
        self.tokens_in.fetch_add(n, Ordering::Relaxed);
    }

    pub fn add_tokens_out(&self, n: u64) {
        self.tokens_out.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc_executions(&self) {
        self.executions.fetch_add(1, Ordering::Relaxed);
    }

    /// Atomically swap counters to zero and return the old values.
    fn drain(&self) -> (u64, u64, u64) {
        (
            self.tokens_in.swap(0, Ordering::Relaxed),
            self.tokens_out.swap(0, Ordering::Relaxed),
            self.executions.swap(0, Ordering::Relaxed),
        )
    }
}

/// Sends periodic heartbeats to AMS with agent runtime state.
///
/// There are two parallel heartbeat channels:
///   * Warden (`/api/warden/heartbeat`) — returns the lifecycle Directive.
///   * Fleet  (`/api/fleet/heartbeat`)  — populates the in-memory
///     container_heartbeats + fleet_registered_agents maps exposed
///     by `/api/fleet/status`.
///
/// Fleet failures are logged but never propagated: warden is the
/// source of truth for lifecycle, fleet is an observability surface.
pub struct HeartbeatReporter {
    ams_client: AmsClient,
    interval_secs: u64,
    tenant_id: String,
    container_id: String,
    started_at: Instant,
    counters: UsageCounters,
}

impl HeartbeatReporter {
    /// Create a new heartbeat reporter.
    pub fn new(ams_client: AmsClient, interval_secs: u64) -> Self {
        Self::with_fleet_context(
            ams_client,
            interval_secs,
            "default".to_string(),
            resolve_container_id(),
            UsageCounters::new(),
        )
    }

    /// Construct with explicit fleet context (used by the runtime so the
    /// tenant id, container id, and shared usage counters are threaded
    /// through from config/env).
    pub fn with_fleet_context(
        ams_client: AmsClient,
        interval_secs: u64,
        tenant_id: String,
        container_id: String,
        counters: UsageCounters,
    ) -> Self {
        Self {
            ams_client,
            interval_secs,
            tenant_id,
            container_id,
            started_at: Instant::now(),
            counters,
        }
    }

    /// Clone of the shared usage counter handle. Hand this to token-accounting
    /// / execution tracking so they can bump counters without re-entering the
    /// reporter.
    pub fn counters(&self) -> UsageCounters {
        self.counters.clone()
    }

    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Send a heartbeat with current runtime state and return the AMS directive.
    ///
    /// AMS evaluates context_pct against governance thresholds and returns
    /// one of: Continue, Warn, BeginDeathRitual, HardStop.
    ///
    /// A fleet heartbeat is fired in parallel with the warden call, and its
    /// result is logged-and-ignored so a fleet outage cannot break lifecycle.
    pub async fn tick(&self, state: &RuntimeState) -> Result<Directive> {
        debug!(
            agent_id = %state.agent_id,
            context_pct = state.context_pct,
            status = %state.status,
            "Sending heartbeat to AMS"
        );

        let warden_payload = HeartbeatPayload {
            agent_id: state.agent_id.clone(),
            status: state.status.clone(),
            context_pct: state.context_pct,
            execution_id: state.current_execution.clone(),
            metadata: None,
        };

        let fleet_payload = self.build_fleet_payload(state);

        let (warden_res, fleet_res) = tokio::join!(
            self.ams_client.heartbeat(warden_payload),
            self.ams_client.fleet_heartbeat(&fleet_payload),
        );

        match fleet_res {
            Ok(resp) => debug!(
                ok = resp.ok,
                received = ?resp.received,
                "Fleet heartbeat accepted"
            ),
            Err(err) => warn!(
                error = %err,
                agent_id = %state.agent_id,
                "Fleet heartbeat failed — continuing"
            ),
        }

        let response = warden_res?;

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

    fn build_fleet_payload(&self, state: &RuntimeState) -> FleetHeartbeatRequest {
        let uptime_secs = self.started_at.elapsed().as_secs();
        let sys = SystemMetrics::collect(uptime_secs);
        let (tokens_in, tokens_out, executions) = self.counters.drain();

        FleetHeartbeatRequest {
            agent_id: state.agent_id.clone(),
            tenant_id: self.tenant_id.clone(),
            container_id: self.container_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            status: normalize_fleet_status(&state.status),
            metrics: FleetHeartbeatMetrics {
                memory_usage_mb: sys.ram_mb.round().max(0.0) as u64,
                cpu_percent: sys.cpu_pct.round().clamp(0.0, 100.0) as u32,
                uptime_seconds: uptime_secs,
                pending_tasks: 0,
                context_usage_percent: state.context_pct as f32,
            },
            usage: FleetHeartbeatUsage {
                tokens_in_since_last_heartbeat: tokens_in,
                tokens_out_since_last_heartbeat: tokens_out,
                executions_since_last_heartbeat: executions,
            },
        }
    }
}

/// Map the runtime status enum to the narrower fleet vocabulary
/// (`idle` | `working` | `error`) expected by app/api/fleet.py.
fn normalize_fleet_status(status: &str) -> String {
    let s = status.to_ascii_lowercase();
    match s.as_str() {
        "working" | "running" | "busy" | "executing" => "working".to_string(),
        "error" | "failed" | "crashed" => "error".to_string(),
        _ => "idle".to_string(),
    }
}

/// Best-effort container id resolution. Prefers an explicit env var
/// (`ABOT_CONTAINER_ID` / `HOSTNAME`), falls back to the kernel hostname,
/// finally the literal `"unknown"`.
fn resolve_container_id() -> String {
    if let Ok(id) = std::env::var("ABOT_CONTAINER_ID")
        && !id.is_empty()
    {
        return id;
    }
    if let Ok(id) = std::env::var("HOSTNAME")
        && !id.is_empty()
    {
        return id;
    }
    match std::fs::read_to_string("/etc/hostname") {
        Ok(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                "unknown".to_string()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => "unknown".to_string(),
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

    #[test]
    fn usage_counters_drain_is_atomic() {
        let c = UsageCounters::new();
        c.add_tokens_in(100);
        c.add_tokens_out(50);
        c.inc_executions();
        c.inc_executions();

        let (ti, to, ex) = c.drain();
        assert_eq!(ti, 100);
        assert_eq!(to, 50);
        assert_eq!(ex, 2);

        let (ti2, to2, ex2) = c.drain();
        assert_eq!((ti2, to2, ex2), (0, 0, 0));
    }

    #[test]
    fn normalize_status_maps_aliases() {
        assert_eq!(normalize_fleet_status("running"), "working");
        assert_eq!(normalize_fleet_status("WORKING"), "working");
        assert_eq!(normalize_fleet_status("error"), "error");
        assert_eq!(normalize_fleet_status("crashed"), "error");
        assert_eq!(normalize_fleet_status("idle"), "idle");
        assert_eq!(normalize_fleet_status(""), "idle");
        assert_eq!(normalize_fleet_status("whatever"), "idle");
    }
}
