use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::{GlobTool, GrepTool, ListDirectoryTool, ReadFileTool, Tool, ToolProvider};

pub struct ReadOnlyToolProvider {
    working_directory: PathBuf,
}

impl ReadOnlyToolProvider {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }
}

impl ToolProvider for ReadOnlyToolProvider {
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>> {
        // Provide ONLY read-only tools for subagent analysis
        vec![
            Arc::new(ReadFileTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(ListDirectoryTool::with_working_directory(
                self.working_directory.clone(),
            )),
            Arc::new(GlobTool::new()),
            Arc::new(GrepTool::new()),
        ]
    }

    fn provider_name(&self) -> &'static str {
        "readonly"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readonly_provider_only_read_tools() {
        let provider = ReadOnlyToolProvider::new(PathBuf::from("."));
        let tools = provider.provide_tools();

        // Should provide exactly 4 read-only tools
        assert_eq!(tools.len(), 4);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();

        // Verify read-only tools present
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"grep"));
        assert!(tool_names.contains(&"glob"));

        // Verify write tools NOT included
        assert!(!tool_names.contains(&"write_file"));
        assert!(!tool_names.contains(&"edit_file"));
        assert!(!tool_names.contains(&"bash"));
        assert!(!tool_names.contains(&"task"));
    }

    #[test]
    fn test_readonly_provider_name() {
        let provider = ReadOnlyToolProvider::new(PathBuf::from("."));
        assert_eq!(provider.provider_name(), "readonly");
    }
}
