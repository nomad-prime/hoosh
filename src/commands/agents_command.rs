use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct AgentsCommand;

#[async_trait]
impl Command for AgentsCommand {
    fn name(&self) -> &str {
        "agents"
    }

    fn description(&self) -> &str {
        "List available agents"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["a"]
    }

    fn usage(&self) -> &str {
        "/agents\n\nLists all available agents that can be used."
    }

    async fn execute(
        &self,
        _args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        if let Some(agent_manager) = &context.agent_manager {
            let agents = agent_manager.list_agents();
            let mut output = String::from(" ⎿ Available Agents:\n\n");

            let current_agent = context.current_agent_name.as_deref();

            for agent in agents {
                // Mark current agent with an indicator
                let marker = if Some(agent.name.as_str()) == current_agent {
                    "→ "
                } else {
                    "  "
                };

                output.push_str(&format!("{}• {}\n", marker, agent.name));
                if let Some(ref desc) = agent.description {
                    output.push_str(&format!("    {}\n", desc));
                }
            }

            Ok(CommandResult::Success(output))
        } else {
            Err(anyhow!("Agent manager not available"))
        }
    }
}
