use serde::{Deserialize, Serialize};
use tracing::warn;

/// Heartbeat payload sent to AMS every tick.
/// The abot reports state; AMS makes decisions.
///
/// BOLT OPTIMIZATION: Uses borrowed references (`&'a str`) instead of owned `String`s
/// to prevent costly memory allocations (`.clone()`) on every frequent telemetry tick.
#[derive(Debug, Serialize)]
pub struct HeartbeatPayload<'a> {
    pub agent_id: &'a str,
    pub status: &'a str,
    pub context_pct: f64,
    pub execution_id: Option<&'a str>,
    pub metadata: Option<&'a serde_json::Value>,
}

/// Raw AMS heartbeat response — the actual wire format from warden.py.
///
/// AMS returns `action`-oriented payloads, not the clean `Directive` enum.
/// Current known action values from `_evaluate_context()`:
///   - null           → context < 85%, normal operation
///   - "remind"       → 85-95%, warning zone
///   - "begin_death_ritual" → 95-98%, critical zone
///   - "pause_and_queue"    → 98%+, hard stop
#[derive(Debug, Deserialize)]
pub struct AmsHeartbeatResponse {
    pub state: Option<String>,
    pub action: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub final_chance: bool,
    pub governance: Option<serde_json::Value>,
}

/// Clean directive enum for internal use in the Rust runtime.
/// Mapped from AMS `action` values via `AmsHeartbeatResponse::to_directive()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Directive {
    /// Keep working, everything fine. (action: null)
    Continue,
    /// Context getting high, be mindful. (action: "remind")
    Warn,
    /// Start death ritual now. (action: "begin_death_ritual")
    BeginDeathRitual,
    /// Emergency stop — death ritual immediately. (action: "pause_and_queue")
    HardStop,
}

impl AmsHeartbeatResponse {
    /// Map AMS action-oriented response to clean Directive enum.
    pub fn to_directive(&self) -> Directive {
        match self.action.as_deref() {
            None => Directive::Continue,
            Some("remind") => Directive::Warn,
            Some("begin_death_ritual") => Directive::BeginDeathRitual,
            Some("pause_and_queue") => Directive::HardStop,
            Some(unknown) => {
                warn!(action = unknown, "Unknown AMS action, treating as Continue");
                Directive::Continue
            }
        }
    }
}

/// Wrapper that presents the clean interface to the runtime.
/// Constructed from the raw AMS response.
#[derive(Debug)]
pub struct HeartbeatResponse {
    pub directive: Directive,
    pub message: Option<String>,
    pub governance: Option<serde_json::Value>,
    pub raw: AmsHeartbeatResponse,
}

impl From<AmsHeartbeatResponse> for HeartbeatResponse {
    fn from(raw: AmsHeartbeatResponse) -> Self {
        let directive = raw.to_directive();
        Self {
            directive,
            message: raw.message.clone(),
            governance: raw.governance.clone(),
            raw,
        }
    }
}

/// Birth ritual request.
#[derive(Debug, Serialize)]
pub struct BirthRequest {
    pub agent_id: String,
    pub agent_name: String,
    pub metadata: serde_json::Value,
}

/// Birth ritual response — may include a pending continuation to resume.
///
/// AMS returns: { ok, agent_id, registered, grants, continuation }
///
/// `grants` contains AMS-authoritative operating limits. The body MUST
/// respect these — they cannot be overridden locally. A forked body that
/// ignores grants will be flagged by AMS governance.
#[derive(Debug, Deserialize)]
pub struct BirthResponse {
    pub ok: bool,
    pub agent_id: String,
    pub registered: bool,
    pub grants: Option<AmsGrants>,
    pub continuation: Option<ContinuationClaim>,
}

/// AMS-granted operating limits — the head's canonical authority.
///
/// These come from the seeded agent database, not from the body's
/// HAND.toml. The body sends claims (who it is); AMS returns grants
/// (what it's allowed to do).
#[derive(Debug, Clone, Deserialize)]
pub struct AmsGrants {
    pub trust_tier: u8,
    pub agent_class: String,
    #[serde(default)]
    pub warn_threshold: u8,
    #[serde(default)]
    pub critical_threshold: u8,
    #[serde(default)]
    pub nanny_managed: bool,
    #[serde(default)]
    pub enable_tools: bool,
    #[serde(default)]
    pub max_iterations: u32,
    #[serde(default)]
    pub default_model: Option<String>,
}

/// A continuation claimed during birth ritual.
///
/// AMS returns continuation_id (not id), and completed_subtasks alongside remaining.
#[derive(Debug, Deserialize)]
pub struct ContinuationClaim {
    pub continuation_id: String,
    pub original_goal: String,
    pub next_action: String,
    #[serde(default)]
    pub completed_subtasks: Vec<String>,
    #[serde(default)]
    pub remaining_subtasks: Vec<String>,
    #[serde(default)]
    pub priority_memories: Vec<String>,
    pub handoff_notes: Option<String>,
    #[serde(default)]
    pub chain_depth: u32,
}

/// Death ritual request.
///
/// Fields match AMS DeathRitualRequest Pydantic model exactly:
/// agent_id, original_goal, next_action, completed_subtasks, remaining_subtasks,
/// handoff_notes, context_pct, memories.
#[derive(Debug, Serialize)]
pub struct DeathRequest {
    pub agent_id: String,
    pub original_goal: String,
    pub next_action: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub completed_subtasks: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remaining_subtasks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_notes: Option<String>,
    pub context_pct: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<MemoryCrystal>,
}

/// A memory to crystallize during death ritual.
#[derive(Debug, Serialize)]
pub struct MemoryCrystal {
    pub title: String,
    pub content: String,
    pub memory_tier: String,
    pub importance: f64,
    pub tags: Vec<String>,
}

/// Death ritual response.
///
/// AMS returns: { ok: bool, agent_id: str, continuation_id: str | null, memories_saved: int }
#[derive(Debug, Deserialize)]
pub struct DeathResponse {
    pub ok: bool,
    pub agent_id: String,
    pub continuation_id: Option<String>,
    #[serde(default)]
    pub memories_saved: u32,
}
