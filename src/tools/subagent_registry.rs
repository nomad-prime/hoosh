use std::path::Path;
use std::sync::Arc;

use crate::task_management::AgentType;
use crate::tools::{BuiltinToolProvider, ReadOnlyToolProvider, ToolProvider, ToolRegistry};

pub fn create_subagent_registry(
    agent_type: &AgentType,
    working_directory: &Path,
) -> Arc<ToolRegistry> {
    let provider: Arc<dyn ToolProvider> = match agent_type {
        AgentType::Plan | AgentType::Explore | AgentType::Review => {
            Arc::new(ReadOnlyToolProvider::new(working_directory.to_path_buf()))
        }
        AgentType::General => Arc::new(BuiltinToolProvider::new(working_directory.to_path_buf())),
    };

    Arc::new(ToolRegistry::new().with_provider(provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_names(agent_type: &AgentType) -> Vec<String> {
        let registry = create_subagent_registry(agent_type, Path::new("."));
        registry
            .list_tools()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect()
    }

    #[test]
    fn read_only_agents_get_no_bash_or_write() {
        for agent_type in [AgentType::Plan, AgentType::Explore, AgentType::Review] {
            let names = tool_names(&agent_type);
            assert!(names.contains(&"read_file".to_string()));
            assert!(names.contains(&"grep".to_string()));
            assert!(
                !names.contains(&"bash".to_string()),
                "{agent_type:?} must not have bash"
            );
            assert!(!names.contains(&"write_file".to_string()));
            assert!(!names.contains(&"edit_file".to_string()));
            assert!(!names.contains(&"task".to_string()));
        }
    }

    #[test]
    fn general_agent_gets_full_coding_tools() {
        let names = tool_names(&AgentType::General);
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"write_file".to_string()));
        assert!(names.contains(&"edit_file".to_string()));
        assert!(names.contains(&"bash".to_string()));
        assert!(
            !names.contains(&"task".to_string()),
            "subagents must not spawn further subagents"
        );
    }
}
