use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;
use tracing::{debug, warn};

use crate::provider::LlmResponse;

/// Mode for Kilo invocation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KiloMode {
    Code,
    Architect,
    Debug,
    Ask,
    Orchestrator,
}

impl KiloMode {
    /// Get the CLI flag for this mode
    pub fn as_flag(&self) -> &str {
        match self {
            Self::Code => "--code",
            Self::Architect => "--architect",
            Self::Debug => "--debug",
            Self::Ask => "--ask",
            Self::Orchestrator => "--orchestrator",
        }
    }
}

/// Error types for Kilo operations
#[derive(Error, Debug)]
pub enum KiloError {
    #[error("Kilo binary not found")]
    BinaryNotFound,

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    #[error("Subprocess error: {0}")]
    SubprocessError(String),
}

/// Bridge to invoke Kilo CLI as subprocess
pub struct KiloBridge {
    kilo_path: String,
}

impl KiloBridge {
    /// Create a new Kilo bridge
    ///
    /// # Arguments
    /// * `kilo_path` - Path to the kilo binary (defaults to "kilo")
    pub fn new(kilo_path: Option<String>) -> Self {
        Self {
            kilo_path: kilo_path.unwrap_or_else(|| "kilo".to_string()),
        }
    }

    /// Execute a prompt with the specified mode
    pub fn execute(&self, prompt: &str, mode: KiloMode) -> Result<LlmResponse, KiloError> {
        debug!(
            mode = ?mode,
            prompt_len = prompt.len(),
            "Invoking Kilo"
        );

        let output = Command::new(&self.kilo_path)
            .arg(mode.as_flag())
            .arg("--")
            .arg(prompt)
            .output()
            .map_err(|e| {
                warn!("Failed to execute kilo: {}", e);
                KiloError::BinaryNotFound
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(KiloError::ExecutionFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| KiloError::InvalidOutput(format!("Invalid UTF-8: {}", e)))?;

        // Parse Kilo output (simplified - actual format depends on Kilo)
        let (content, tokens_used) = Self::parse_output(&stdout)?;

        Ok(LlmResponse {
            content,
            model_used: format!("kilo-{:?}", mode).to_lowercase(),
            tokens_used,
        })
    }

    /// Parse output from Kilo subprocess
    fn parse_output(output: &str) -> Result<(String, u64), KiloError> {
        // Simple parsing: expect lines like "TOKENS: 123" and rest is content
        let mut tokens_used = 0u64;
        let mut content_lines = Vec::new();

        for line in output.lines() {
            if line.starts_with("TOKENS:") {
                tokens_used = line
                    .strip_prefix("TOKENS:")
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
            } else if !line.is_empty() {
                content_lines.push(line);
            }
        }

        let content = content_lines.join("\n");
        if content.is_empty() {
            return Err(KiloError::InvalidOutput("No content in output".to_string()));
        }

        Ok((content, tokens_used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kilo_mode_flags() {
        assert_eq!(KiloMode::Code.as_flag(), "--code");
        assert_eq!(KiloMode::Ask.as_flag(), "--ask");
        assert_eq!(KiloMode::Architect.as_flag(), "--architect");
    }

    #[test]
    fn test_parse_output() {
        let output = "TOKENS: 42\nThis is the generated content";
        let (content, tokens) = KiloBridge::parse_output(output).unwrap();

        assert_eq!(content, "This is the generated content");
        assert_eq!(tokens, 42);
    }

    #[test]
    fn test_parse_output_multiline() {
        let output = "TOKENS: 100\nLine 1\nLine 2\nLine 3";
        let (content, tokens) = KiloBridge::parse_output(output).unwrap();

        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 3"));
        assert_eq!(tokens, 100);
    }

    #[test]
    fn test_kilo_bridge_creation() {
        let bridge = KiloBridge::new(None);
        assert_eq!(bridge.kilo_path, "kilo");

        let bridge = KiloBridge::new(Some("/usr/local/bin/kilo".to_string()));
        assert_eq!(bridge.kilo_path, "/usr/local/bin/kilo");
    }
}
