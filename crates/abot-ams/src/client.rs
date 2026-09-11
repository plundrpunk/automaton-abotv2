use anyhow::Result;
use reqwest::Client;
use reqwest::Method;
use reqwest::StatusCode;
use std::time::Duration;
use tracing::debug;
use urlencoding::encode;

use crate::fleet::{
    ExecutionChunkRequest, ExecutionChunkResponse, FleetHeartbeatRequest, FleetHeartbeatResponse,
    FleetRegisterAgentRequest, FleetRegisterAgentResponse, RegisterExecutionRequest,
    RegisterExecutionResponse,
};
use crate::llm::{
    CompletionRequest, CompletionResponse, ToolCompletionRequest, ToolCompletionResponse,
};
use crate::warden::{AmsHeartbeatResponse, HeartbeatPayload, HeartbeatResponse};
use crate::warden::{BirthRequest, BirthResponse, DeathRequest, DeathResponse};

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
    pub async fn heartbeat(&self, payload: HeartbeatPayload<'_>) -> Result<HeartbeatResponse> {
        let url = format!("{}/api/warden/heartbeat", self.base_url);
        debug!(url = %url, agent_id = %payload.agent_id, "Sending heartbeat");

        let raw = self
            .request(Method::POST, url)
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

        let resp = self
            .request(Method::POST, url)
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

        let resp = self
            .request(Method::POST, url)
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

        let resp = self
            .request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<SteeringMessage>>()
            .await?;

        Ok(resp)
    }

    pub async fn register_execution(
        &self,
        request: &RegisterExecutionRequest<'_>,
    ) -> Result<RegisterExecutionResponse> {
        let url = format!("{}/api/fleet/executions/register", self.base_url);
        let resp = self
            .request(Method::POST, url)
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
        chunk: &ExecutionChunkRequest<'_>,
    ) -> Result<ExecutionChunkResponse> {
        let url = execution_chunk_url(&self.base_url, execution_id);
        let resp = self
            .request(Method::POST, url)
            .json(chunk)
            .send()
            .await?
            .error_for_status()?
            .json::<ExecutionChunkResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn complete(&self, request: &CompletionRequest<'_>) -> Result<CompletionResponse> {
        let url = format!("{}/api/v1/llm/complete", self.base_url);
        let timeout_ms = self.request_timeout_ms.max(180_000);
        let resp = self
            .request(Method::POST, url)
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
    ///
    /// Returns the raw JSON so the caller is insulated from the
    /// MemoryResponse schema drifting on the server side. AMS requires the
    /// trailing slash; without it the server responds 307 and reqwest drops
    /// the request body before following.
    pub async fn create_memory(
        &self,
        memory: crate::memory::CreateMemoryRequest,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/memories/", self.base_url);

        let resp = self
            .request(Method::POST, url)
            .json(&memory)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        Ok(resp)
    }

    /// Search memories via hybrid search.
    ///
    /// Returns the raw `results` array (each element has `memory`,
    /// `relevance_score`, `content_snippet`) so the caller doesn't have to
    /// track server schema drift. AMS requires the trailing slash on search
    /// as well.
    pub async fn search_memories(&self, query: &str, limit: u32) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/api/v1/memories/search/", self.base_url);

        let resp: serde_json::Value = self
            .request(Method::POST, url)
            .json(&serde_json::json!({
                "query": query,
                "limit": limit,
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let results = resp
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(results)
    }

    /// LLM completion with tool calling support.
    pub async fn complete_with_tools(
        &self,
        request: &ToolCompletionRequest<'_>,
    ) -> Result<ToolCompletionResponse> {
        let url = format!("{}/api/v1/llm/complete-with-tools", self.base_url);
        let timeout_ms = self.request_timeout_ms.max(180_000);
        let resp = self
            .request(Method::POST, url)
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
    ///
    /// `metadata` is a generic correlation blob; AMS stores and returns it
    /// verbatim. See [`SteeringMessage::metadata`] for the per-`msg_type`
    /// contract used across the swarm (task dispatch, rollup, etc).
    pub async fn send_steering_message(
        &self,
        agent_id: &str,
        content: &str,
        msg_type: &str,
        sender: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/api/warden/agents/{}/messages",
            self.base_url,
            urlencoding::encode(agent_id)
        );
        let mut payload = serde_json::json!({
            "type": msg_type,
            "content": content,
            "sender": sender,
        });
        if let Some(md) = metadata {
            payload["metadata"] = md.clone();
        }
        let resp = self
            .request(Method::POST, url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    /// Post an assistant message to a dashboard chat session.
    ///
    /// Thin wrapper over AMS internal
    /// `POST /api/v1/chat/sessions/{id}/assistant-message` endpoint which
    /// persists the message into the `chat_messages` table that the
    /// dashboard UI polls. Called after `run_tool_loop` completes when the
    /// activation carries a `chat_session_id` — this is what lets a
    /// rollup-triggered synthesis turn show up in the same dashboard
    /// conversation that initiated the original dispatch.
    pub async fn post_chat_message(
        &self,
        session_id: &str,
        content: &str,
        source_agent_id: &str,
        model: Option<&str>,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/api/v1/chat/sessions/{}/assistant-message",
            self.base_url,
            urlencoding::encode(session_id)
        );
        let mut payload = serde_json::json!({
            "content": content,
            "source_agent_id": source_agent_id,
        });
        if let Some(m) = model {
            payload["model"] = serde_json::Value::String(m.to_string());
        }
        let resp = self
            .request(Method::POST, url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    /// List agents with v3-body capability.
    ///
    /// AMS returns a wrapper object `{"agents": [...], "count": N}` from
    /// `/api/v1/agents`. We unwrap that here so callers keep getting a flat
    /// array of agent records. Default server limit is 50; we explicitly ask
    /// for 500 to cover the full hydrated fleet.
    pub async fn list_worker_agents(&self) -> Result<Vec<serde_json::Value>> {
        self.list_worker_agents_filtered(None).await
    }

    /// List agents whose `agent_name` starts with the given prefix.
    ///
    /// Domain-scoped TLs (e.g. `tl-engineering`) use this to discover only the
    /// specialists registered under their domain (e.g. `engineering-`),
    /// instead of seeing the entire fleet roster. When `prefix` is `None` this
    /// behaves like [`list_worker_agents`] and returns every agent.
    pub async fn list_worker_agents_filtered(
        &self,
        prefix: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        let url = match prefix {
            Some(p) if !p.is_empty() => format!(
                "{}/api/v1/agents?limit=500&name_prefix={}",
                self.base_url,
                urlencoding::encode(p),
            ),
            _ => format!("{}/api/v1/agents?limit=500", self.base_url),
        };
        let resp: serde_json::Value = self
            .request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let agents = resp
            .get("agents")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(agents)
    }

    /// Create a goal task for DLPFC to route via NEXUS.
    pub async fn create_goal_task(
        &self,
        title: &str,
        description: &str,
        priority: &str,
        creator_agent_id: &str,
    ) -> Result<String> {
        let url = format!("{}/api/v1/goals", self.base_url);
        let payload = serde_json::json!({
            "title": title,
            "description": description,
            "priority": priority,
            "created_by": creator_agent_id,
            "source": "abot-orchestrator",
        });
        let resp = self
            .request(Method::POST, url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(resp)
    }

    /// Read the AMS goals dashboard/task board.
    pub async fn get_task_board(&self) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/goals/dashboard", self.base_url);
        let resp = self
            .request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    /// Fleet heartbeat — populates the in-memory container_heartbeats +
    /// fleet_registered_agents maps in `app/api/fleet.py`. This is what
    /// surfaces agents on `/api/fleet/status`.
    pub async fn fleet_heartbeat<'a>(
        &self,
        payload: &FleetHeartbeatRequest<'a>,
    ) -> Result<FleetHeartbeatResponse> {
        let url = format!("{}/api/fleet/heartbeat", self.base_url);
        debug!(url = %url, agent_id = %payload.agent_id, "Sending fleet heartbeat");
        let resp = self
            .request(Method::POST, url)
            .json(payload)
            .send()
            .await?
            .error_for_status()?
            .json::<FleetHeartbeatResponse>()
            .await?;
        Ok(resp)
    }

    /// Fleet agent registration — idempotent one-time call at birth.
    pub async fn fleet_register_agent(
        &self,
        payload: &FleetRegisterAgentRequest<'_>,
    ) -> Result<FleetRegisterAgentResponse> {
        let url = format!("{}/api/fleet/agents", self.base_url);
        debug!(url = %url, agent_id = %payload.agent_id, "Registering fleet agent");
        let resp = self
            .request(Method::POST, url)
            .json(payload)
            .send()
            .await?
            .error_for_status()?
            .json::<FleetRegisterAgentResponse>()
            .await?;
        Ok(resp)
    }

    /// Fetch an observatory execution by id. Used by the orchestrator
    /// dispatch_and_wait loop to block on a worker's terminal state.
    /// Returns the parsed JSON body including `status`, `output`, and
    /// `duration_ms` when the row exists; caller retries transient 404s.
    pub async fn get_execution(&self, execution_id: &str) -> Result<serde_json::Value> {
        let url = format!(
            "{}/observatory/executions/{}",
            self.base_url,
            urlencoding::encode(execution_id)
        );
        let resp = self
            .request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    /// Find a child Observatory execution by Warden dispatch correlation.
    ///
    /// Alive specialists only receive a queued Warden message immediately;
    /// their execution id is created later when they poll and register. This
    /// lookup bridges that gap for TL fan-in.
    pub async fn find_execution_by_lineage(
        &self,
        correlation_id: Option<&str>,
        parent_execution_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> Result<Option<serde_json::Value>> {
        let mut url = format!("{}/observatory/lookup/executions?limit=20", self.base_url);
        if let Some(corr) = correlation_id {
            url.push_str("&correlation_id=");
            url.push_str(&urlencoding::encode(corr));
        }
        if let Some(parent) = parent_execution_id {
            url.push_str("&parent_execution_id=");
            url.push_str(&urlencoding::encode(parent));
        }
        if let Some(agent) = agent_id {
            url.push_str("&agent_id=");
            url.push_str(&urlencoding::encode(agent));
        }
        let resp = self
            .request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp
            .get("executions")
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .cloned())
    }

    /// List MCP servers currently known by AMS MCP Gateway.
    pub async fn mcp_list_servers(&self) -> Result<serde_json::Value> {
        let url = format!("{}/gateway/mcp/servers", self.base_url);
        let resp = self
            .request(Method::GET, url)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        Ok(resp)
    }

    /// Execute a tool on an MCP server through AMS MCP Gateway.
    pub async fn mcp_call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: &serde_json::Value,
        timeout_seconds: f64,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/gateway/mcp/call", self.base_url);
        let timeout = timeout_seconds.clamp(1.0, 300.0);
        // Let the gateway finish and return its response before HTTP times out.
        let request_timeout = Duration::from_millis(self.request_timeout_ms)
            .max(Duration::from_secs_f64(timeout) + Duration::from_secs(5));
        let payload = serde_json::json!({
            "server": server,
            "tool": tool,
            "args": arguments,
            "timeout_seconds": timeout,
        });

        let resp = self
            .request(Method::POST, url)
            .timeout(request_timeout)
            .json(&payload)
            .send()
            .await?;

        // If gateway endpoints are not present yet on AMS, return a structured
        // capability error instead of hard-failing the agent turn.
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(serde_json::json!({
                "success": false,
                "error": "gateway endpoint not found (/gateway/mcp/call)",
                "server": server,
                "tool": tool,
            }));
        }

        let resp = resp.error_for_status()?;
        let data = resp.json::<serde_json::Value>().await?;
        Ok(data)
    }

    /// Health check.
    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self.request(Method::GET, url).send().await?;
        Ok(resp.status().is_success())
    }
}

fn execution_chunk_url(base_url: &str, execution_id: &str) -> String {
    format!(
        "{}/api/fleet/executions/{}/emit",
        base_url,
        encode(execution_id)
    )
}

/// A steering message from AMS (queued by DLPFC, admin, or system).
fn default_guidance_type() -> String {
    "guidance".to_string()
}
fn default_dashboard_sender() -> String {
    "dashboard".to_string()
}
fn default_agent_recipient() -> String {
    "agent".to_string()
}

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
    /// Generic per-message correlation blob. Passed through AMS verbatim.
    ///
    /// Expected keys by `msg_type` (contract lives here, not in schema):
    /// - `task` (orchestrator -> TL via dispatch_to_tl):
    ///   {parent_exec_id, parent_agent_id, chat_session_id?}
    /// - `rollup` (TL -> orchestrator on completion):
    ///   {parent_exec_id, child_exec_id, child_agent_id, memory_id,
    ///   chat_session_id?}
    /// - `guidance` / `intervention` / `command`: may carry
    ///   {chat_session_id?} when the user-facing dashboard initiated the
    ///   turn, so the runtime can post its synthesized reply back into
    ///   that chat session on completion.
    ///
    /// `chat_session_id` rides alongside the agent-to-agent correlation
    /// keys so a rollup-triggered synthesis turn lands back in the
    /// originating dashboard conversation instead of being orphaned.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
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
#[derive(Clone)]
pub struct AmsConfig {
    pub url: String,
    pub api_key: String,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub heartbeat_interval_secs: u64,
}

impl std::fmt::Debug for AmsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmsConfig")
            .field("url", &self.url)
            .field("api_key", &"***")
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("request_timeout_ms", &self.request_timeout_ms)
            .field("heartbeat_interval_secs", &self.heartbeat_interval_secs)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{AmsClient, AmsConfig, SteeringMessage, execution_chunk_url};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn mcp_call_outlives_client_default_and_sends_clamped_timeout() {
        for (requested, expected) in [(120.0, 120.0), (0.0, 1.0), (600.0, 300.0)] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let payload = loop {
                    let mut chunk = [0; 1024];
                    let count = socket.read(&mut chunk).await.unwrap();
                    assert_ne!(count, 0, "request ended before body arrived");
                    request.extend_from_slice(&chunk[..count]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&request[..end]).unwrap();
                        assert!(headers.starts_with("POST /gateway/mcp/call HTTP/1.1"));
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse().unwrap())
                            })
                            .unwrap();
                        if request.len() >= end + 4 + length {
                            break serde_json::from_slice::<serde_json::Value>(
                                &request[end + 4..end + 4 + length],
                            )
                            .unwrap();
                        }
                    }
                };
                assert_eq!(payload["timeout_seconds"], expected);
                assert_eq!(payload["server"], "test-server");
                assert_eq!(payload["tool"], "test-tool");
                assert_eq!(payload["args"], serde_json::json!({"input": "test"}));
                // Longer than the client's 50 ms default, shorter than the tool budget.
                tokio::time::sleep(Duration::from_millis(200)).await;
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"success\":true}").await.unwrap();
            });
            let client = AmsClient::new(&AmsConfig {
                url: format!("http://{address}"),
                api_key: String::new(),
                connect_timeout_ms: 1000,
                request_timeout_ms: 50,
                heartbeat_interval_secs: 60,
            })
            .unwrap();
            let result = tokio::time::timeout(
                Duration::from_secs(3),
                client.mcp_call_tool(
                    "test-server",
                    "test-tool",
                    &serde_json::json!({"input": "test"}),
                    requested,
                ),
            )
            .await
            .unwrap();
            server.await.unwrap();
            assert_eq!(result.unwrap(), serde_json::json!({"success": true}));
        }
    }

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

    #[test]
    fn execution_chunk_url_encodes_path_segments() {
        let url = execution_chunk_url("https://ams.example", "../chunk id");
        assert_eq!(
            url,
            "https://ams.example/api/fleet/executions/..%2Fchunk%20id/emit"
        );
    }
}
