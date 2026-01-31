use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct ToolsCommand;

#[async_trait]
impl Command for ToolsCommand {
    fn name(&self) -> &str {
        "tools"
    }

    fn description(&self) -> &str {
        "List available tools"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["t"]
    }

    fn usage(&self) -> &str {
        "/tools\n\nLists all available tools that can be used during conversations."
    }

    async fn execute(
        &self,
        _args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        if let Some(tool_registry) = &context.tool_registry {
            let tools = tool_registry.list_tools();
            let mut output = String::from("🛠️  Available Tools:\n\n");

            for (name, description) in tools {
                let first_line = description.lines().next().unwrap_or("");
                output.push_str(&format!("- **{}** - {}\n", name, first_line));
            }

            Ok(CommandResult::Success(output))
        } else {
            Err(anyhow!("Tool registry not available"))
        }
    }
}
