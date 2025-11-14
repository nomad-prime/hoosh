use std::path::PathBuf;
use std::sync::Arc;

use crate::tools::bash_blacklist::BlacklistFile;
use crate::tools::{
    BashTool, EditFileTool, GrepTool, ListDirectoryTool, ReadFileTool, Tool, WriteFileTool,
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
}

impl BuiltinToolProvider {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }
}

impl ToolProvider for BuiltinToolProvider {
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>> {
        // Create .hoosh/bash_blacklist.json if it doesn't exist
        if let Err(e) = BlacklistFile::create_default_if_missing(&self.working_directory) {
            eprintln!("Warning: Failed to create bash blacklist file: {}", e);
        }

        // Load blacklist patterns
        let blacklist_patterns = BlacklistFile::load_safe(&self.working_directory);

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
            Arc::new(GrepTool::new()),
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

        assert_eq!(tools.len(), 6);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"edit_file"));
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"bash"));
        assert!(tool_names.contains(&"grep"));
    }

    #[test]
    fn test_builtin_provider_name() {
        let provider = BuiltinToolProvider::new(PathBuf::from("."));
        assert_eq!(provider.provider_name(), "builtin");
    }
}
