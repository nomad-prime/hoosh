use anyhow::Result;
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct StatusCommand;

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

    async fn execute(
        &self,
        _args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let mut status = String::from("Session Status\n\n");

        status.push_str(&format!(
            "Working Directory: {}\n",
            context.working_directory
        ));

        if let Some(conv) = &context.conversation {
            let conv = conv.lock().await;
            let message_count = conv.messages.len();
            status.push_str(&format!("Conversation Messages: {}\n", message_count));
        }

        if let Some(tool_registry) = &context.tool_registry {
            let tool_count = tool_registry.list_tools().len();
            status.push_str(&format!("Available Tools: {}\n", tool_count));
        }

        // Show current agent instead of default agent
        if let Some(current_agent) = &context.current_agent_name {
            status.push_str(&format!("Current Agent: {}\n", current_agent));
        }

        Ok(CommandResult::Success(status))
    }
}
