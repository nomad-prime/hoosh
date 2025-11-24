use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::todo_state::TodoState;
use crate::tools::{
    BashTool, EditFileTool, GlobTool, GrepTool, ListDirectoryTool, ReadFileTool, TodoWriteTool,
    Tool, WriteFileTool,
};

/// Trait for tool providers that can register tools dynamically
pub trait ToolProvider: Send + Sync {
    /// Get all tools provided by this provider
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Provider name for debugging/logging
    fn provider_name(&self) -> &'static str;
}

/// Built-in tools provider that provides standard file and bash
pub struct BuiltinToolProvider {
    working_directory: PathBuf,
    todo_state: TodoState,
}

impl BuiltinToolProvider {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            todo_state: TodoState::new(),
        }
    }

    pub fn with_todo_state(working_directory: PathBuf, todo_state: TodoState) -> Self {
        Self {
            working_directory,
            todo_state,
        }
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
            Arc::new(BashTool::new().with_working_directory(self.working_directory.clone()).with_timeout(120)),
            Arc::new(GlobTool::new()),
            Arc::new(GrepTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(TodoWriteTool::new(self.todo_state.clone())),
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

        assert_eq!(tools.len(), 8);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"edit_file"));
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"bash"));
        assert!(tool_names.contains(&"glob"));
        assert!(tool_names.contains(&"grep"));
        assert!(tool_names.contains(&"todo_write"));
    }

    #[test]
    fn test_builtin_provider_name() {
        let provider = BuiltinToolProvider::new(PathBuf::from("."));
        assert_eq!(provider.provider_name(), "builtin");
    }
}
