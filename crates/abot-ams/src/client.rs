use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tracing::debug;

use crate::warden::{BirthRequest, BirthResponse, DeathRequest, DeathResponse};
use crate::warden::{AmsHeartbeatResponse, HeartbeatPayload, HeartbeatResponse};

/// HTTP client for communicating with AMS backend.
/// The abot is a dumb body — AMS is the brain.
#[derive(Clone)]
pub struct AmsClient {
    client: Client,
    base_url: String,
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
        })
    }

    /// Send heartbeat to AMS Warden. Returns directive.
    ///
    /// AMS returns action-oriented responses (remind, begin_death_ritual,
    /// pause_and_queue). We deserialize the raw format and convert to
    /// the clean Directive enum via the adapter layer.
    pub async fn heartbeat(&self, payload: HeartbeatPayload) -> Result<HeartbeatResponse> {
        let url = format!("{}/api/warden/heartbeat", self.base_url);
        debug!(url = %url, agent_id = %payload.agent_id, "Sending heartbeat");

        let raw = self.client
            .post(&url)
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

        let resp = self.client
            .post(&url)
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

        let resp = self.client
            .post(&url)
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
        let url = format!("{}/api/warden/agents/{}/messages", self.base_url, agent_id);

        let resp = self.client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<SteeringMessage>>()
            .await?;

        Ok(resp)
    }

    /// Create an episodic memory in AMS.
    pub async fn create_memory(&self, memory: crate::memory::CreateMemoryRequest) -> Result<crate::memory::MemoryResponse> {
        let url = format!("{}/api/v1/memories", self.base_url);

        let resp = self.client
            .post(&url)
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

        let resp = self.client
            .post(&url)
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

    /// Health check.
    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self.client.get(&url).send().await?;
        Ok(resp.status().is_success())
    }
}

/// A steering message from AMS (queued by DLPFC, admin, or system).
#[derive(Debug, serde::Deserialize)]
pub struct SteeringMessage {
    pub msg_type: String,
    pub content: serde_json::Value,
    pub sender: String,
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
