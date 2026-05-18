use serde::{Deserialize, Serialize};

/// Register execution payload.
///
/// BOLT OPTIMIZATION: Uses borrowed references (`&'a str`) instead of owned `String`s
/// to prevent costly memory allocations (`.clone()`) on every request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterExecutionRequest<'a> {
    pub agent_id: &'a str,
    pub tenant_id: &'a str,
    pub execution_id: &'a str,
    pub agent_name: &'a str,
    pub task: &'a str,
    pub model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<&'a str>,
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
pub struct ExecutionChunkRequest<'a> {
    pub agent_id: &'a str,
    pub tenant_id: &'a str,
    pub execution_id: &'a str,
    #[serde(rename = "type")]
    pub chunk_type: &'a str,
    pub timestamp: &'a str,
    pub data: ExecutionChunkData<'a>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionChunkData<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'a str>,
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
pub struct FleetHeartbeatRequest<'a> {
    pub agent_id: &'a str,
    pub tenant_id: &'a str,
    pub container_id: &'a str,
    pub timestamp: &'a str,
    pub status: &'a str,
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
