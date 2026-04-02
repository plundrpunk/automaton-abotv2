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
