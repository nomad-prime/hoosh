use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

use super::registry::{Command, CommandContext, CommandRegistry, CommandResult};

struct HelpCommand;

#[async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "Show available commands and usage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["h", "?"]
    }

    fn usage(&self) -> &str {
        "/help [command]\n\nShow help for all commands or a specific command."
    }

    async fn execute(&self, args: Vec<String>, context: &mut CommandContext) -> Result<CommandResult> {
        let help_text = if args.is_empty() {
            if let Some(registry) = &context.command_registry {
                registry.get_help(None)
            } else {
                "Command registry not available".to_string()
            }
        } else {
            if let Some(registry) = &context.command_registry {
                registry.get_help(Some(&args[0]))
            } else {
                format!("Command registry not available")
            }
        };

        Ok(CommandResult::Success(help_text))
    }
}

struct ClearCommand;

#[async_trait]
impl Command for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "Clear conversation history"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["c"]
    }

    fn usage(&self) -> &str {
        "/clear\n\nClears the current conversation history, starting a fresh session."
    }

    async fn execute(&self, _args: Vec<String>, _context: &mut CommandContext) -> Result<CommandResult> {
        Ok(CommandResult::ClearConversation)
    }
}

struct StatusCommand;

#[async_trait]
impl Command for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }

    fn description(&self) -> &str {
        "Show current session status"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["s"]
    }

    fn usage(&self) -> &str {
        "/status\n\nDisplays information about the current session, including working directory and conversation state."
    }

    async fn execute(&self, _args: Vec<String>, context: &mut CommandContext) -> Result<CommandResult> {
        let mut status = String::from("📊 Session Status\n\n");

        status.push_str(&format!("Working Directory: {}\n", context.working_directory));

        if let Some(conv) = &context.conversation {
            let conv = conv.lock().await;
            let message_count = conv.messages.len();
            status.push_str(&format!("Conversation Messages: {}\n", message_count));
        }

        if let Some(tool_registry) = &context.tool_registry {
            let tool_count = tool_registry.list_tools().len();
            status.push_str(&format!("Available Tools: {}\n", tool_count));
        }

        if let Some(agent_manager) = &context.agent_manager {
            if let Some(default_agent) = agent_manager.get_default_agent() {
                status.push_str(&format!("Current Agent: {}\n", default_agent.name));
            }
        }

        Ok(CommandResult::Success(status))
    }
}

struct ToolsCommand;

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

    async fn execute(&self, _args: Vec<String>, context: &mut CommandContext) -> Result<CommandResult> {
        if let Some(tool_registry) = &context.tool_registry {
            let tools = tool_registry.list_tools();
            let mut output = String::from("🛠️  Available Tools:\n\n");

            for (name, description) in tools {
                output.push_str(&format!("  • {}\n", name));
                output.push_str(&format!("    {}\n", description));
            }

            Ok(CommandResult::Success(output))
        } else {
            Err(anyhow!("Tool registry not available"))
        }
    }
}

struct AgentsCommand;

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

    async fn execute(&self, _args: Vec<String>, context: &mut CommandContext) -> Result<CommandResult> {
        if let Some(agent_manager) = &context.agent_manager {
            let agents = agent_manager.list_agents();
            let mut output = String::from("🤖 Available Agents:\n\n");

            for agent in agents {
                output.push_str(&format!("  • {}\n", agent.name));
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

struct ExitCommand;

#[async_trait]
impl Command for ExitCommand {
    fn name(&self) -> &str {
        "exit"
    }

    fn description(&self) -> &str {
        "Exit the application"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["quit", "q"]
    }

    fn usage(&self) -> &str {
        "/exit\n\nExits the application."
    }

    async fn execute(&self, _args: Vec<String>, _context: &mut CommandContext) -> Result<CommandResult> {
        Ok(CommandResult::Exit)
    }
}

pub fn register_default_commands(registry: &mut CommandRegistry) -> Result<()> {
    registry.register(Arc::new(HelpCommand))?;
    registry.register(Arc::new(ClearCommand))?;
    registry.register(Arc::new(StatusCommand))?;
    registry.register(Arc::new(ToolsCommand))?;
    registry.register(Arc::new(AgentsCommand))?;
    registry.register(Arc::new(ExitCommand))?;
    Ok(())
}
