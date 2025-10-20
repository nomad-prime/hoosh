use anyhow::{anyhow, Result};
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};
use crate::conversations::AgentEvent;

pub struct SwitchAgentCommand;

#[async_trait]
impl Command for SwitchAgentCommand {
    fn name(&self) -> &str {
        "switch-agent"
    }

    fn description(&self) -> &str {
        "Switch to a different agent"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sa"]
    }

    fn usage(&self) -> &str {
        "/switch-agent <agent-name> [--set-default]\n\nSwitches to the specified agent. Use /agents to see available agents.\nAdd --set-default to also update the default agent in config."
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        // Check if agent name provided
        if args.is_empty() {
            return Err(anyhow!(
                "Agent name required. Usage: /switch-agent <agent-name> [--set-default]\nUse /agents to see available agents."
            ));
        }

        let mut set_default = false;
        let new_agent_name = if args.len() > 1 && args[1] == "--set-default" {
            set_default = true;
            args[0].clone()
        } else if args.len() > 1 {
            return Err(anyhow!(
                "Invalid arguments. Usage: /switch-agent <agent-name> [--set-default]"
            ));
        } else {
            args[0].clone()
        };

        // Get agent manager
        let agent_manager = context
            .agent_manager
            .as_ref()
            .ok_or_else(|| anyhow!("Agent manager not available"))?;

        // Check if already using this agent
        if let Some(current_agent) = &context.current_agent_name {
            if current_agent == &new_agent_name {
                let mut message = format!("Already using {} agent", new_agent_name);
                if set_default {
                    message.push_str(" (default unchanged)");
                }
                return Ok(CommandResult::Success(message));
            }
        }

        // Get the new agent
        let new_agent = agent_manager.get_agent(&new_agent_name).ok_or_else(|| {
            let available_agents = agent_manager
                .list_agents()
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow!(
                "Agent '{}' not found. Available agents: {}",
                new_agent_name,
                available_agents
            )
        })?;

        // Update the conversation's system message
        if let Some(conv) = &context.conversation {
            let mut conv = conv.lock().await;

            // Update the first message (system prompt)
            if let Some(first_msg) = conv.messages.first_mut() {
                if first_msg.role == "system" {
                    first_msg.content = Some(new_agent.content.clone());
                }
            } else {
                // If no system message exists, add one
                conv.add_system_message(new_agent.content.clone());
            }
        }

        // Send event to update the event loop context
        if let Some(event_tx) = &context.event_tx {
            let _ = event_tx.send(AgentEvent::AgentSwitched {
                new_agent_name: new_agent_name.clone(),
            });
        }

        // Update default agent in config if requested
        let mut message = format!("Switched to {} agent", new_agent_name);
        if set_default {
            if context.config.is_some() {
                // Load the current config from file to ensure we have the latest version
                let mut file_config = crate::config::AppConfig::load()
                    .map_err(|e| anyhow!("Failed to load config: {}", e))?;
                
                // Update the default agent
                file_config.set_default_agent(new_agent_name.clone());
                
                // Save the config
                file_config
                    .save()
                    .map_err(|e| anyhow!("Failed to save config: {}", e))?;
                
                message.push_str(" (set as default)");
            } else {
                message.push_str(" (warning: could not update default config)");
            }
        }

        Ok(CommandResult::Success(message))
    }
}
