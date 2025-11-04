use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::agent::Conversation;
use crate::agent_definition::AgentDefinitionManager;
use crate::config::AppConfig;
use crate::context_management::{ContextManager, MessageSummarizer};
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
    pub agent_manager: Option<Arc<AgentDefinitionManager>>,
    pub command_registry: Option<Arc<CommandRegistry>>,
    pub working_directory: String,
    pub permission_manager: Option<Arc<crate::permissions::PermissionManager>>,
    pub summarizer: Option<Arc<MessageSummarizer>>,
    pub current_agent_name: Option<String>,
    pub event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::agent::AgentEvent>>,
    pub config: Option<AppConfig>,
    pub backend: Option<Arc<dyn crate::backends::LlmBackend>>,
    pub context_manager: Option<Arc<ContextManager>>,
}

impl CommandContext {
    pub fn new() -> Self {
        Self {
            conversation: None,
            tool_registry: None,
            agent_manager: None,
            command_registry: None,
            working_directory: String::new(),
            permission_manager: None,
            summarizer: None,
            current_agent_name: None,
            event_tx: None,
            config: None,
            backend: None,
            context_manager: None,
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

    pub fn with_agent_manager(mut self, manager: Arc<AgentDefinitionManager>) -> Self {
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

    pub fn with_permission_manager(
        mut self,
        manager: Arc<crate::permissions::PermissionManager>,
    ) -> Self {
        self.permission_manager = Some(manager);
        self
    }

    pub fn with_summarizer(mut self, summarizer: Arc<MessageSummarizer>) -> Self {
        self.summarizer = Some(summarizer);
        self
    }

    pub fn with_current_agent_name(mut self, name: String) -> Self {
        self.current_agent_name = Some(name);
        self
    }

    pub fn with_event_sender(
        mut self,
        tx: tokio::sync::mpsc::UnboundedSender<crate::agent::AgentEvent>,
    ) -> Self {
        self.event_tx = Some(tx);
        self
    }

    pub fn with_config(mut self, config: AppConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_backend(mut self, backend: Arc<dyn crate::backends::LlmBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn with_context_manager(mut self, context_manager: Arc<ContextManager>) -> Self {
        self.context_manager = Some(context_manager);
        self
    }
}

impl Default for CommandContext {
    fn default() -> Self {
        Self::new()
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
