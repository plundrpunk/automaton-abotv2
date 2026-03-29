use serde::{Deserialize, Serialize};

/// Request to create a memory in AMS.
#[derive(Debug, Serialize)]
pub struct CreateMemoryRequest {
    pub title: String,
    pub content: String,
    pub memory_tier: String,
    pub entity_type: String,
    pub importance: f64,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Memory response from AMS.
#[derive(Debug, Deserialize)]
pub struct MemoryResponse {
    pub memory_id: String,
    pub title: String,
    pub content: String,
    pub memory_tier: String,
    pub entity_type: String,
    pub importance: f64,
    pub tags: Vec<String>,
    pub created_at: String,
    pub similarity: Option<f64>,
}
