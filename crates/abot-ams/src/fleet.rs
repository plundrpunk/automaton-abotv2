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

// ---------------------------------------------------------------------------
// Fleet heartbeat + registration (matches ams-client.ts:131/147 and
// app/api/fleet.py:384/432 server contract).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FleetHeartbeatMetrics {
    pub memory_usage_mb: u64,
    pub cpu_percent: u32,
    pub uptime_seconds: u64,
    #[serde(default)]
    pub pending_tasks: u32,
    #[serde(default)]
    pub context_usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FleetHeartbeatUsage {
    #[serde(default)]
    pub tokens_in_since_last_heartbeat: u64,
    #[serde(default)]
    pub tokens_out_since_last_heartbeat: u64,
    #[serde(default)]
    pub executions_since_last_heartbeat: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetHeartbeatRequest {
    pub agent_id: String,
    pub tenant_id: String,
    pub container_id: String,
    pub timestamp: String,
    pub status: String,
    pub metrics: FleetHeartbeatMetrics,
    pub usage: FleetHeartbeatUsage,
}

#[derive(Debug, Deserialize)]
pub struct FleetHeartbeatResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub received: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetRegisterAgentRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct FleetRegisterAgentResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub registered_at: Option<String>,
}
