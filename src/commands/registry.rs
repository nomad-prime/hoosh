use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::agents::AgentManager;
use crate::conversations::Conversation;
use crate::tools::ToolRegistry;

#[derive(Debug, Clone)]
pub enum CommandResult {
    Success(String),
    Exit,
    ClearConversation,
}

pub struct CommandContext {
    pub conversation: Option<Arc<tokio::sync::Mutex<Conversation>>>,
    pub tool_registry: Option<Arc<ToolRegistry>>,
    pub agent_manager: Option<Arc<AgentManager>>,
    pub command_registry: Option<Arc<CommandRegistry>>,
    pub working_directory: String,
}

impl CommandContext {
    pub fn new() -> Self {
        Self {
            conversation: None,
            tool_registry: None,
            agent_manager: None,
            command_registry: None,
            working_directory: String::new(),
        }
    }

    pub fn with_conversation(mut self, conv: Arc<tokio::sync::Mutex<Conversation>>) -> Self {
        self.conversation = Some(conv);
        self
    }

    pub fn with_tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn with_agent_manager(mut self, manager: Arc<AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    pub fn with_command_registry(mut self, registry: Arc<CommandRegistry>) -> Self {
        self.command_registry = Some(registry);
        self
    }

    pub fn with_working_directory(mut self, dir: String) -> Self {
        self.working_directory = dir;
        self
    }
}

#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> Vec<&str> {
        Vec::new()
    }
    fn usage(&self) -> &str;
    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult>;
}

pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn register(&mut self, command: Arc<dyn Command>) -> Result<()> {
        let name = command.name().to_string();

        for alias in command.aliases() {
            self.aliases.insert(alias.to_string(), name.clone());
        }

        self.commands.insert(name, command);
        Ok(())
    }

    pub async fn execute(
        &self,
        input: &str,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let input = input.trim();

        if !input.starts_with('/') {
            return Err(anyhow!("Command must start with '/'"));
        }

        let parts: Vec<String> = input[1..]
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if parts.is_empty() {
            return Err(anyhow!("Empty command"));
        }

        let cmd_name = &parts[0];
        let args = parts[1..].to_vec();

        let resolved_name = self.aliases.get(cmd_name).unwrap_or(cmd_name);

        let command = self
            .commands
            .get(resolved_name)
            .ok_or_else(|| anyhow!("Unknown command: {}", cmd_name))?;

        command.execute(args, context).await
    }

    pub fn get_help(&self, command_name: Option<&str>) -> String {
        if let Some(name) = command_name {
            let resolved_name = self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);

            if let Some(command) = self.commands.get(resolved_name) {
                format!(
                    "{} - {}\n\nUsage: {}",
                    command.name(),
                    command.description(),
                    command.usage()
                )
            } else {
                format!("Unknown command: {}", name)
            }
        } else {
            let mut help = String::from("Available commands:\n\n");
            let mut commands: Vec<_> = self.commands.values().collect();
            commands.sort_by_key(|c| c.name());

            for command in commands {
                help.push_str(&format!(
                    "  /{:<12} {}\n",
                    command.name(),
                    command.description()
                ));
            }

            help.push_str("\nType /help <command> for more information about a specific command.");
            help
        }
    }

    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        self.commands
            .values()
            .map(|cmd| (cmd.name(), cmd.description()))
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
