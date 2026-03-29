use serde::{Deserialize, Serialize};

/// Request to create a continuation (usually during death ritual via Warden).
#[derive(Debug, Serialize)]
pub struct CreateContinuationRequest {
    pub agent_id: String,
    pub session_id: String,
    pub original_goal: String,
    pub next_action: String,
    pub completed_subtasks: Vec<String>,
    pub remaining_subtasks: Vec<String>,
    pub priority_memories: Vec<String>,
    pub handoff_notes: Option<String>,
    pub project: Option<String>,
    pub task_type: Option<String>,
}

/// Continuation state from AMS.
#[derive(Debug, Deserialize)]
pub struct ContinuationState {
    pub id: String,
    pub status: String,
    pub original_goal: String,
    pub next_action: String,
    pub remaining_subtasks: Vec<String>,
    pub priority_memories: Vec<String>,
    pub chain_depth: u32,
    pub created_at: String,
    pub expires_at: String,
}
