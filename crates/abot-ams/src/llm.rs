use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CompletionRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CompletionResponse {
    pub text: String,
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub latency_ms: f64,
    pub cost: f64,
    pub request_id: String,
}

/// Request for LLM completion with tool calling.
#[derive(Debug, Serialize)]
pub struct ToolCompletionRequest<'a> {
    pub messages: &'a [serde_json::Value],
    pub tools: &'a [serde_json::Value],
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

/// Response from LLM tool completion.
#[derive(Debug, Deserialize)]
pub struct ToolCompletionResponse {
    pub text: String,
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub tool_calls: Vec<serde_json::Value>,
    #[serde(default = "default_stop")]
    pub finish_reason: String,
    pub request_id: String,
}

fn default_stop() -> String {
    "stop".to_string()
}
