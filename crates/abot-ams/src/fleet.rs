use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterExecutionRequest {
    pub agent_id: String,
    pub tenant_id: String,
    pub execution_id: String,
    pub agent_name: String,
    pub task: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterExecutionResponse {
    pub ok: bool,
    pub fleet_execution_id: String,
    pub execution_id: String,
    pub agent_id: String,
    pub reused: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionChunkRequest {
    pub agent_id: String,
    pub tenant_id: String,
    pub execution_id: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub timestamp: String,
    pub data: ExecutionChunkData,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionChunkData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionChunkResponse {
    pub ok: bool,
    #[serde(rename = "chunk_type", alias = "chunkType")]
    pub chunk_type: String,
    #[serde(rename = "executionId", alias = "execution_id")]
    pub execution_id: String,
}
