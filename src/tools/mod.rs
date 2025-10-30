use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

use crate::permissions::OperationType;

/// Core trait for all tools in the hoosh system
#[async_trait]
pub trait Tool: Send + Sync {
    /// Execute the tool with the given arguments
    async fn execute(&self, args: &Value) -> Result<String>;

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

    fn to_operation_type(&self, args: &Option<Value>) -> Result<OperationType>;

    async fn generate_preview(&self, _args: &Value) -> Option<String> {
        None
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
pub mod error;
pub mod file_ops;
pub mod provider;

pub use bash::BashTool;
pub use error::{ToolError, ToolResult};
pub use file_ops::{EditFileTool, ListDirectoryTool, ReadFileTool, WriteFileTool};
pub use provider::{BuiltinToolProvider, ToolProvider};

/// Tool registry for managing available tools through providers
#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<&'static str, Arc<dyn Tool>>,
    providers: Vec<Arc<dyn ToolProvider>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            providers: Vec::new(),
        }
    }

    /// Register a tool provider and load its tools
    pub fn with_provider(mut self, provider: Arc<dyn ToolProvider>) -> Self {
        self.add_provider(provider);
        self
    }

    /// Add a provider and register its tools
    pub fn add_provider(&mut self, provider: Arc<dyn ToolProvider>) {
        // Get tools from provider and register them
        for tool in provider.provide_tools() {
            let name = tool.tool_name();
            if self.tools.contains_key(name) {
                eprintln!(
                    "Warning: Tool '{}' already registered, skipping from provider '{}'",
                    name,
                    provider.provider_name()
                );
                continue;
            }
            self.tools.insert(name, tool);
        }
        self.providers.push(provider);
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
        self.tools.values().map(|tool| tool.tool_schema()).collect()
    }

    /// Refresh tools from all providers (useful for dynamic tools)
    pub fn refresh(&mut self) {
        self.tools.clear();
        let providers = std::mem::take(&mut self.providers);
        for provider in providers {
            for tool in provider.provide_tools() {
                let name = tool.tool_name();
                if self.tools.contains_key(name) {
                    eprintln!(
                        "Warning: Tool '{}' already registered, skipping from provider '{}'",
                        name,
                        provider.provider_name()
                    );
                    continue;
                }
                self.tools.insert(name, tool);
            }
            self.providers.push(provider);
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new().with_provider(Arc::new(BuiltinToolProvider::new(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        )))
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

        async fn execute(&self, _args: &Value) -> Result<String> {
            Ok(self.response.to_string())
        }

        fn to_operation_type(&self, _args: &Option<Value>) -> Result<OperationType> {
            Ok(OperationType::new("mock"))
        }
    }

    struct MockToolProvider {
        tools: Vec<Arc<dyn Tool>>,
    }

    impl MockToolProvider {
        fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
            Self { tools }
        }
    }

    impl ToolProvider for MockToolProvider {
        fn provide_tools(&self) -> Vec<Arc<dyn Tool>> {
            self.tools.clone()
        }

        fn provider_name(&self) -> &'static str {
            "mock"
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
        let mut registry = ToolRegistry::new();
        registry
            .register_tool(Arc::new(MockTool::new(
                "mock_tool",
                "Mock tool",
                "Mock response",
            )))
            .expect("Failed to register tool");

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
        let mut registry = ToolRegistry::new();
        registry
            .register_tool(Arc::new(MockTool::new(
                "mock_tool",
                "Mock tool",
                "Mock response",
            )))
            .expect("Failed to register tool");
        registry
            .register_tool(Arc::new(MockTool::new(
                "another_mock_tool",
                "Another mock tool",
                "Another mock response",
            )))
            .expect("Failed to register tool");

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
    fn test_tool_registry_with_same_tool() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool::new("mock_tool", "Mock tool", "Mock response"));

        // First registration should succeed
        registry
            .register_tool(tool.clone())
            .expect("First registration should succeed");

        // Second registration should fail
        let result = registry.register_tool(tool);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Tool with name 'mock_tool' already exists")
        );
    }

    #[test]
    fn test_tool_registry_with_provider() {
        let mock_tool = Arc::new(MockTool::new("mock_tool", "Mock tool", "Mock response"));
        let provider = Arc::new(MockToolProvider::new(vec![mock_tool.clone()]));

        let registry = ToolRegistry::new().with_provider(provider);

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
    fn test_tool_registry_with_multiple_providers() {
        let mock_tool1 = Arc::new(MockTool::new(
            "mock_tool1",
            "Mock tool 1",
            "Mock response 1",
        ));
        let mock_tool2 = Arc::new(MockTool::new(
            "mock_tool2",
            "Mock tool 2",
            "Mock response 2",
        ));

        let provider1 = Arc::new(MockToolProvider::new(vec![mock_tool1.clone()]));
        let provider2 = Arc::new(MockToolProvider::new(vec![mock_tool2.clone()]));

        let registry = ToolRegistry::new()
            .with_provider(provider1)
            .with_provider(provider2);

        // Both tools should be registered
        assert!(registry.get_tool("mock_tool1").is_some());
        assert!(registry.get_tool("mock_tool2").is_some());

        // List all tools
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_tool_registry_refresh() {
        let mock_tool1 = Arc::new(MockTool::new(
            "mock_tool1",
            "Mock tool 1",
            "Mock response 1",
        ));
        let provider = Arc::new(MockToolProvider::new(vec![mock_tool1.clone()]));

        let mut registry = ToolRegistry::new().with_provider(provider);

        // Initially should have 1 tool
        assert_eq!(registry.list_tools().len(), 1);

        // Refresh should keep the same tools
        registry.refresh();
        assert_eq!(registry.list_tools().len(), 1);
    }
}
