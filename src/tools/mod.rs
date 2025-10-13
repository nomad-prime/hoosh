use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

use crate::permissions::PermissionManager;

/// Core trait for all tools in the hoosh system
#[async_trait]
pub trait Tool: Send + Sync {
    /// Execute the tool with the given arguments
    async fn execute(&self, args: &serde_json::Value) -> Result<String>;

    /// Get the tool's name (used for identification)
    fn tool_name(&self) -> &'static str;

    /// Get a description of what this tool does
    fn description(&self) -> &'static str;

    /// Get the parameter schema for this tool (JSON Schema format)
    fn parameter_schema(&self) -> Value;

    /// Format the tool call for display (e.g., "Read(src/main.rs)")
    /// This is shown when the tool is invoked
    fn format_call_display(&self, _args: &Value) -> String {
        // Default implementation: just return tool name
        self.tool_name().to_string()
    }

    /// Create a summary of the tool execution result for display
    /// This allows each tool to format its own output summary
    fn result_summary(&self, result: &str) -> String {
        // Default implementation: show first line or generic message
        let preview = result.lines().next().unwrap_or("");
        if preview.len() > 60 {
            format!("{}...", &preview[..60])
        } else if !preview.is_empty() {
            preview.to_string()
        } else {
            "Completed successfully".to_string()
        }
    }

    /// Check if this tool execution is permitted
    /// Each tool knows how to request its own permissions
    /// Default implementation allows all operations (for safe/readonly tools)
    async fn check_permission(
        &self,
        _args: &serde_json::Value,
        _permission_manager: &PermissionManager,
    ) -> Result<bool> {
        // Default: no permission needed (safe operations)
        Ok(true)
    }

    /// Get the complete tool schema in OpenAI function calling format
    fn tool_schema(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.tool_name(),
                "description": self.description(),
                "parameters": self.parameter_schema()
            }
        })
    }
}

pub mod bash;
pub mod file_ops;

pub use bash::BashTool;
pub use file_ops::{ListDirectoryTool, ReadFileTool, WriteFileTool};

/// Tool registry for managing available tools
#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<&'static str, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn with_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.register_tool(tool).expect("Failed to register tool");
        self
    }

    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) -> Result<(), String> {
        let name = tool.tool_name();
        if self.tools.contains_key(name) {
            return Err(format!("Tool with name '{}' already exists", name));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|tool| tool.as_ref())
    }

    pub fn list_tools(&self) -> Vec<(&str, &str)> {
        self.tools
            .iter()
            .map(|(name, tool)| (*name, tool.description()))
            .collect()
    }

    pub fn get_tool_schemas(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|tool| tool.tool_schema())
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
            .with_tool(Arc::new(ReadFileTool::new()))
            .with_tool(Arc::new(WriteFileTool::new()))
            .with_tool(Arc::new(ListDirectoryTool::new()))
            .with_tool(Arc::new(BashTool::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct MockTool {
        name: &'static str,
        description: &'static str,
        response: &'static str,
    }

    impl MockTool {
        fn new(name: &'static str, description: &'static str, response: &'static str) -> Self {
            Self {
                name,
                description,
                response,
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn tool_name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            self.description
        }

        fn parameter_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }

        async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
            Ok(self.response.to_string())
        }
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();

        // Register a new tool
        let _ = registry.register_tool(Arc::new(MockTool::new(
            "mock_tool",
            "Mock tool",
            "Mock response",
        )));

        // Get the tool by name
        let tool = registry
            .get_tool("mock_tool")
            .expect("mock_tool should exist, but it did not");
        assert_eq!(tool.tool_name(), "mock_tool");
        assert_eq!(tool.description(), "Mock tool");

        // List all tools
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn test_tool_registry_with_tool() {
        // Register a new tool
        let registry = ToolRegistry::new().with_tool(Arc::new(MockTool::new(
            "mock_tool",
            "Mock tool",
            "Mock response",
        )));

        // Get the tool by name
        let tool = registry
            .get_tool("mock_tool")
            .expect("mock_tool should exist, but it did not");
        assert_eq!(tool.tool_name(), "mock_tool");
        assert_eq!(tool.description(), "Mock tool");

        // List all tools
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn test_tool_registry_with_tool_chain() {
        let registry = ToolRegistry::new()
            .with_tool(Arc::new(MockTool::new(
                "mock_tool",
                "Mock tool",
                "Mock response",
            )))
            .with_tool(Arc::new(MockTool::new(
                "another_mock_tool",
                "Another mock tool",
                "Another mock response",
            )));

        // Get the tool by name
        let tool = registry
            .get_tool("mock_tool")
            .expect("mock_tool should exist, but it did not");
        assert_eq!(tool.tool_name(), "mock_tool");
        assert_eq!(tool.description(), "Mock tool");

        // List all tools
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    #[should_panic(
        expected = "Failed to register tool: \"Tool with name 'mock_tool' already exists\""
    )]
    fn test_tool_registry_with_same_tool() {
        let _registry = ToolRegistry::new()
            .with_tool(Arc::new(MockTool::new(
                "mock_tool",
                "Mock tool",
                "Mock response",
            )))
            .with_tool(Arc::new(MockTool::new(
                "mock_tool",
                "Mock tool2",
                "Mock response3",
            )));
    }
}
