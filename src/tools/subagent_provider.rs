use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::bash_blacklist::BlacklistFile;
use crate::tools::{BashTool, EditFileTool, GlobTool, GrepTool, ListDirectoryTool, ReadFileTool, Tool, ToolProvider, WriteFileTool};

/// Tool provider for sub-agents that provides all standard tools except the Task tool.
/// This prevents infinite recursion where sub-agents spawn more sub-agents.
pub struct SubAgentToolProvider {
    working_directory: PathBuf,
}

impl SubAgentToolProvider {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }
}

impl ToolProvider for SubAgentToolProvider {
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>> {
        // Create .hoosh/bash_blacklist.json if it doesn't exist
        if let Err(e) = BlacklistFile::create_default_if_missing(&self.working_directory) {
            eprintln!("Warning: Failed to create bash blacklist file: {}", e);
        }

        // Load blacklist patterns
        let blacklist_patterns = BlacklistFile::load_safe(&self.working_directory);

        // Provide the same tools as BuiltinToolProvider, but without TaskTool
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
            Arc::new(
                BashTool::with_working_directory(self.working_directory.clone())
                    .with_blacklist(blacklist_patterns),
            ),
            Arc::new(GlobTool::new()),
            Arc::new(GrepTool::new()),
        ]
    }

    fn provider_name(&self) -> &'static str {
        "subagent"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_provider_provides_tools() {
        let provider = SubAgentToolProvider::new(PathBuf::from("."));
        let tools = provider.provide_tools();

        // Should provide 5 tools: read_file, write_file, edit_file, list_directory, bash
        // Notably missing: task tool
        assert_eq!(tools.len(), 5);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"edit_file"));
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"bash"));

        // Verify task tool is NOT included
        assert!(!tool_names.contains(&"task"));
    }

    #[test]
    fn test_subagent_provider_name() {
        let provider = SubAgentToolProvider::new(PathBuf::from("."));
        assert_eq!(provider.provider_name(), "subagent");
    }
}
