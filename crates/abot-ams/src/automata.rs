use serde::{Deserialize, Serialize};

/// Request to execute an automaton via AMS.
#[derive(Debug, Serialize)]
pub struct ExecuteAutomatonRequest {
    pub automaton_id: String,
    pub input: serde_json::Value,
    pub agent_id: String,
}

/// Automaton execution result from AMS.
#[derive(Debug, Deserialize)]
pub struct AutomatonResult {
    pub execution_id: String,
    pub automaton_id: String,
    pub success: bool,
    pub output: serde_json::Value,
    pub duration_ms: u64,
}

/// Automaton suggestion from AMS (semantic + Bayesian ranking).
#[derive(Debug, Deserialize)]
pub struct AutomatonSuggestion {
    pub automaton_id: String,
    pub name: String,
    pub description: String,
    pub success_rate: f64,
    pub relevance_score: f64,
}
