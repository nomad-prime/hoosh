use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::{BashTool, EditFileTool, ListDirectoryTool, ReadFileTool, Tool, WriteFileTool};

/// Trait for tool providers that can register tools dynamically
pub trait ToolProvider: Send + Sync {
    /// Get all tools provided by this provider
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Provider name for debugging/logging
    fn provider_name(&self) -> &'static str;
}

/// Built-in tools provider that provides standard file and bash tools
pub struct BuiltinToolProvider {
    working_directory: PathBuf,
}

impl BuiltinToolProvider {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }
}

impl ToolProvider for BuiltinToolProvider {
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(ReadFileTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(WriteFileTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(EditFileTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(ListDirectoryTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(BashTool::with_working_directory(self.working_directory.clone())),
        ]
    }

    fn provider_name(&self) -> &'static str {
        "builtin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_provider_provides_tools() {
        let provider = BuiltinToolProvider::new(PathBuf::from("."));
        let tools = provider.provide_tools();

        // Should provide 5 tools: read_file, write_file, edit_file, list_directory, bash
        assert_eq!(tools.len(), 5);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.tool_name()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"edit_file"));
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"bash"));
    }

    #[test]
    fn test_builtin_provider_name() {
        let provider = BuiltinToolProvider::new(PathBuf::from("."));
        assert_eq!(provider.provider_name(), "builtin");
    }
}
