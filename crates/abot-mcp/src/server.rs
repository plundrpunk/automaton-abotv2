use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use tracing::debug;

/// Tool definition
#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP (Model Context Protocol) server for exposing agent tools
pub struct McpServer {
    port: u16,
    tools: HashMap<String, ToolDefinition>,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(port: u16) -> Self {
        Self {
            port,
            tools: HashMap::new(),
        }
    }

    /// Register a tool with the server
    pub fn register_tool(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Result<()> {
        let name = name.into();
        debug!(tool = &name, "Registering MCP tool");

        self.tools.insert(
            name.clone(),
            ToolDefinition {
                name,
                description: description.into(),
                input_schema,
            },
        );

        Ok(())
    }

    /// Start the MCP server
    pub async fn start(&self) -> Result<()> {
        debug!(port = self.port, "Starting MCP server");

        // TODO: Implement MCP server startup
        // 1. Create transport (SSE + stdio or WebSocket)
        // 2. Listen for connections
        // 3. Handle tool calls
        // 4. Send responses back to clients

        Ok(())
    }

    /// Stop the MCP server
    pub async fn stop(&self) -> Result<()> {
        debug!(port = self.port, "Stopping MCP server");

        // TODO: Implement graceful shutdown
        // 1. Close all client connections
        // 2. Release port binding
        // 3. Cleanup resources

        Ok(())
    }

    /// Get list of registered tools
    pub fn get_tools(&self) -> Vec<ToolDefinition> {
        self.tools.values().cloned().collect()
    }

    /// Get a specific tool definition
    pub fn get_tool(&self, name: &str) -> Option<ToolDefinition> {
        self.tools.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = McpServer::new(3000);
        assert_eq!(server.port, 3000);
        assert_eq!(server.tools.len(), 0);
    }

    #[test]
    fn test_register_tool() {
        let mut server = McpServer::new(3000);
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            }
        });

        server
            .register_tool("search", "Search for information", schema)
            .unwrap();

        assert_eq!(server.tools.len(), 1);
        assert!(server.get_tool("search").is_some());
    }

    #[test]
    fn test_get_tools() {
        let mut server = McpServer::new(3000);
        let schema = json!({});

        server
            .register_tool("tool1", "First tool", schema.clone())
            .unwrap();
        server
            .register_tool("tool2", "Second tool", schema.clone())
            .unwrap();

        let tools = server.get_tools();
        assert_eq!(tools.len(), 2);
    }
}
