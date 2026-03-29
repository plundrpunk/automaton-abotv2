use async_trait::async_trait;
use thiserror::Error;

/// Error types for channel operations
#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Message structure
#[derive(Clone, Debug)]
pub struct Message {
    pub content: String,
    pub sender: String,
    pub timestamp: String,
    pub metadata: Option<serde_json::Value>,
}

/// Channel trait for unified message platform interface
#[async_trait]
pub trait Channel: Send + Sync {
    /// Connect to the channel
    async fn connect(&mut self) -> Result<(), ChannelError>;

    /// Disconnect from the channel
    async fn disconnect(&mut self) -> Result<(), ChannelError>;

    /// Send a message through the channel
    async fn send_message(&self, content: &str) -> Result<String, ChannelError>;

    /// Receive messages from the channel
    async fn receive_messages(&self) -> Result<Vec<Message>, ChannelError>;

    /// Check if the channel is connected
    fn is_connected(&self) -> bool;

    /// Get the channel name
    fn name(&self) -> &str;
}
