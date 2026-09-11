use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Response from LLM invocation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Generated content
    pub content: String,
    /// Model name used
    pub model_used: String,
    /// Tokens used in generation
    pub tokens_used: u64,
}

/// Error types for LLM operations
#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Timeout")]
    Timeout,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Execute a prompt and get response
    async fn execute(&self, prompt: &str, model: &str) -> Result<LlmResponse, LlmError>;

    /// Check if a model is available
    async fn is_available(&self, model: &str) -> bool;

    /// List available models
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}
