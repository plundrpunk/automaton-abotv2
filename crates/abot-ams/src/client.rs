use anyhow::Result;
use reqwest::Client;
use reqwest::Method;
use std::time::Duration;
use tracing::debug;
use urlencoding::encode;

use crate::fleet::{
    ExecutionChunkRequest,
    ExecutionChunkResponse,
    RegisterExecutionRequest,
    RegisterExecutionResponse,
};
use crate::llm::{CompletionRequest, CompletionResponse, ToolCompletionRequest, ToolCompletionResponse};
use crate::warden::{BirthRequest, BirthResponse, DeathRequest, DeathResponse};
use crate::warden::{AmsHeartbeatResponse, HeartbeatPayload, HeartbeatResponse};

/// HTTP client for communicating with AMS backend.
/// The abot is a dumb body — AMS is the brain.
#[derive(Clone)]
pub struct AmsClient {
    client: Client,
    base_url: String,
    api_key: String,
    request_timeout_ms: u64,
}

impl AmsClient {
    pub fn new(config: &crate::AmsConfig) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .build()?;

        Ok(Self {
            client,
            base_url: config.url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            request_timeout_ms: config.request_timeout_ms,
        })
    }

    fn request(&self, method: Method, url: String) -> reqwest::RequestBuilder {
        let builder = self.client.request(method, url);
        if self.api_key.is_empty() {
            builder
        } else {
            builder.header("X-API-Key", &self.api_key)
        }
    }

    /// Send heartbeat to AMS Warden. Returns directive.
    ///
    /// AMS returns action-oriented responses (remind, begin_death_ritual,
    /// pause_and_queue). We deserialize the raw format and convert to
    /// the clean Directive enum via the adapter layer.
    pub async fn heartbeat(&self, payload: HeartbeatPayload) -> Result<HeartbeatResponse> {
        let url = format!("{}/api/warden/heartbeat", self.base_url);
        debug!(url = %url, agent_id = %payload.agent_id, "Sending heartbeat");

        let raw = self.request(Method::POST, url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<AmsHeartbeatResponse>()
            .await?;

        Ok(HeartbeatResponse::from(raw))
    }

    /// Execute birth ritual — register with AMS, check for continuations.
    pub async fn birth(&self, request: BirthRequest) -> Result<BirthResponse> {
        let url = format!("{}/api/warden/birth", self.base_url);
        debug!(url = %url, agent = %request.agent_name, "Executing birth ritual");

        let resp = self.request(Method::POST, url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<BirthResponse>()
            .await?;

        Ok(resp)
    }

    /// Execute death ritual — save state, create continuation.
    pub async fn death(&self, request: DeathRequest) -> Result<DeathResponse> {
        let url = format!("{}/api/warden/death", self.base_url);
        debug!(url = %url, agent_id = %request.agent_id, "Executing death ritual");

        let resp = self.request(Method::POST, url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<DeathResponse>()
            .await?;

        Ok(resp)
    }

    /// Poll for steering messages from AMS.
    pub async fn poll_messages(&self, agent_id: &str) -> Result<Vec<SteeringMessage>> {
        let url = format!(
            "{}/api/warden/agents/{}/messages",
            self.base_url,
            encode(agent_id)
        );

        let resp = self.request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<SteeringMessage>>()
            .await?;

        Ok(resp)
    }

    pub async fn register_execution(
        &self,
        request: &RegisterExecutionRequest,
    ) -> Result<RegisterExecutionResponse> {
        let url = format!("{}/api/fleet/executions/register", self.base_url);
        let resp = self.request(Method::POST, url)
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<RegisterExecutionResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn emit_execution_chunk(
        &self,
        execution_id: &str,
        chunk: &ExecutionChunkRequest,
    ) -> Result<ExecutionChunkResponse> {
        let url = format!("{}/api/fleet/executions/{}/emit", self.base_url, execution_id);
        let resp = self.request(Method::POST, url)
            .json(chunk)
            .send()
            .await?
            .error_for_status()?
            .json::<ExecutionChunkResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse> {
        let url = format!("{}/api/v1/llm/complete", self.base_url);
        let timeout_ms = self.request_timeout_ms.max(180_000);
        let resp = self.request(Method::POST, url)
            .timeout(Duration::from_millis(timeout_ms))
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<CompletionResponse>()
            .await?;
        Ok(resp)
    }

    /// Create an episodic memory in AMS.
    pub async fn create_memory(&self, memory: crate::memory::CreateMemoryRequest) -> Result<crate::memory::MemoryResponse> {
        let url = format!("{}/api/v1/memories", self.base_url);

        let resp = self.request(Method::POST, url)
            .json(&memory)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp)
    }

    /// Search memories via hybrid search.
    pub async fn search_memories(&self, query: &str, limit: u32) -> Result<Vec<crate::memory::MemoryResponse>> {
        let url = format!("{}/api/v1/memories/search", self.base_url);

        let resp = self.request(Method::POST, url)
            .json(&serde_json::json!({
                "query": query,
                "limit": limit,
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp)
    }


    /// LLM completion with tool calling support.
    pub async fn complete_with_tools(
        &self,
        request: &ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        let url = format!("{}/api/v1/llm/complete-with-tools", self.base_url);
        let timeout_ms = self.request_timeout_ms.max(180_000);
        let resp = self.request(Method::POST, url)
            .timeout(Duration::from_millis(timeout_ms))
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<ToolCompletionResponse>()
            .await?;
        Ok(resp)
    }

    /// Send a steering message to another agent via Warden.
    pub async fn send_steering_message(
        &self,
        agent_id: &str,
        content: &str,
        msg_type: &str,
        sender: &str,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/api/warden/agents/{}/messages",
            self.base_url,
            urlencoding::encode(agent_id)
        );
        let payload = serde_json::json!({
            "type": msg_type,
            "content": content,
            "sender": sender,
        });
        let resp = self.request(Method::POST, url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    /// List agents with v3-body capability.
    pub async fn list_worker_agents(&self) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/api/agents", self.base_url);
        let resp = self.request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<serde_json::Value>>()
            .await?;
        Ok(resp)
    }

    /// Health check.
    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self.request(Method::GET, url).send().await?;
        Ok(resp.status().is_success())
    }
}

/// A steering message from AMS (queued by DLPFC, admin, or system).
fn default_guidance_type() -> String { "guidance".to_string() }
fn default_dashboard_sender() -> String { "dashboard".to_string() }
fn default_agent_recipient() -> String { "agent".to_string() }

#[derive(Debug, serde::Deserialize)]
pub struct SteeringMessage {
    #[serde(default)]
    pub id: String,
    #[serde(alias = "type", default = "default_guidance_type")]
    pub msg_type: String,
    pub content: serde_json::Value,
    #[serde(default = "default_dashboard_sender")]
    pub sender: String,
    #[serde(default = "default_agent_recipient")]
    pub recipient: String,
    #[serde(default)]
    pub timestamp: String,
}

impl SteeringMessage {
    pub fn content_text(&self) -> String {
        match &self.content {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        }
    }
}

/// AMS connection configuration.
#[derive(Debug, Clone)]
pub struct AmsConfig {
    pub url: String,
    pub api_key: String,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub heartbeat_interval_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::SteeringMessage;

    #[test]
    fn steering_message_deserializes_current_warden_shape() {
        let payload = serde_json::json!({
            "id": "msg-123",
            "content": "hello",
            "type": "guidance",
            "sender": "dashboard",
            "recipient": "agent",
            "timestamp": "2026-04-02T19:00:00+00:00"
        });

        let message: SteeringMessage = serde_json::from_value(payload).unwrap();
        assert_eq!(message.id, "msg-123");
        assert_eq!(message.msg_type, "guidance");
        assert_eq!(message.content_text(), "hello");
        assert_eq!(message.sender, "dashboard");
        assert_eq!(message.recipient, "agent");
    }
}
