use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agents::{Agent, AgentManager};
use crate::backends::LlmBackend;
use crate::commands::{register_default_commands, CommandRegistry};
use crate::config::AppConfig;
use crate::conversations::{Conversation, MessageSummarizer};
use crate::parser::MessageParser;
use crate::permissions::PermissionManager;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

use super::app::AppState;
use super::completion::{CommandCompleter, FileCompleter};
use super::event_loop::EventLoopContext;
use super::handlers;
use super::header;
use super::history::PromptHistory;
use super::input_handler::InputHandler;
use super::terminal::{init_terminal, Tui};

pub struct TuiBootstrap {
    backend: Arc<dyn LlmBackend>,
    parser: Arc<MessageParser>,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
    config: AppConfig,
    working_dir: PathBuf,
    working_dir_display: String,
}

impl TuiBootstrap {
    pub fn new(
        backend: Box<dyn LlmBackend>,
        parser: MessageParser,
        permission_manager: PermissionManager,
        tool_registry: ToolRegistry,
        config: AppConfig,
    ) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let working_dir_display = working_dir
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| ".".to_string());

        Self {
            backend: Arc::from(backend),
            parser: Arc::new(parser),
            permission_manager,
            tool_registry,
            config,
            working_dir,
            working_dir_display,
        }
    }

    pub fn init_terminal_and_app(&self) -> Result<(Tui, AppState)> {
        const BASE_UI_HEIGHT: u16 = 6;

        let terminal = init_terminal(BASE_UI_HEIGHT).context("Failed to initialize terminal")?;
        let mut app = AppState::new();

        if let Some(history_path) = PromptHistory::default_history_path() {
            if let Ok(history) = PromptHistory::with_file(1000, &history_path) {
                app.prompt_history = history;
            }
        }

        Ok((terminal, app))
    }

    pub fn setup_completers(&self, app: &mut AppState) -> Result<()> {
        let file_completer = FileCompleter::new(self.working_dir.clone());
        app.register_completer(Box::new(file_completer));

        let mut command_registry = CommandRegistry::new();
        register_default_commands(&mut command_registry)
            .context("Failed to register default commands")?;
        let command_registry = Arc::new(command_registry);
        let command_completer = CommandCompleter::new(Arc::clone(&command_registry));
        app.register_completer(Box::new(command_completer));

        Ok(())
    }

    pub fn setup_agents(&self) -> Result<Arc<AgentManager>> {
        let agent_manager = AgentManager::new().context("Failed to initialize agent manager")?;
        Ok(Arc::new(agent_manager))
    }

    pub fn setup_header(&self, app: &mut AppState, default_agent: &Option<Agent>) -> Result<()> {
        let agent_name = default_agent.as_ref().map(|a| a.name.as_str());
        for line in header::create_header_block(
            self.backend.backend_name(),
            self.backend.model_name(),
            &self.working_dir_display,
            agent_name,
            None,
        ) {
            app.add_styled_line(line);
        }

        if !self.permission_manager.is_enforcing() {
            app.add_message("⚠️ Permission checks disabled (--skip-permissions)".to_string());
        }

        app.add_message("\n".to_string());

        Ok(())
    }

    pub fn setup_conversation(
        &self,
        default_agent: Option<&Agent>,
    ) -> Result<Arc<tokio::sync::Mutex<Conversation>>> {
        let conversation = Arc::new(tokio::sync::Mutex::new({
            let mut conv = Conversation::new();
            if let Some(agent) = default_agent {
                conv.add_system_message(agent.content.clone());
            }
            conv
        }));

        Ok(conversation)
    }

    pub fn setup_channels(
        &self,
    ) -> Result<(
        mpsc::UnboundedSender<crate::conversations::AgentEvent>,
        mpsc::UnboundedReceiver<crate::conversations::AgentEvent>,
        Arc<PermissionManager>,
    )> {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_permission_response_tx, permission_response_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let permission_manager = self
            .permission_manager
            .clone()
            .with_event_sender(event_tx.clone())
            .with_response_receiver(permission_response_rx);

        let permission_manager_arc = Arc::new(permission_manager);

        Ok((event_tx, event_rx, permission_manager_arc))
    }

    pub fn setup_tool_executor(
        &self,
        app: &AppState,
        _event_tx: &mpsc::UnboundedSender<crate::conversations::AgentEvent>,
    ) -> Result<Arc<ToolExecutor>> {
        let (_approval_response_tx, approval_response_rx) = tokio::sync::mpsc::unbounded_channel();

        let tool_executor =
            ToolExecutor::new(self.tool_registry.clone(), self.permission_manager.clone())
                .with_event_sender(_event_tx.clone())
                .with_autopilot_state(Arc::clone(&app.autopilot_enabled))
                .with_approval_receiver(approval_response_rx);

        Ok(Arc::new(tool_executor))
    }

    pub fn setup_input_handlers(
        &self,
        _event_tx: &mpsc::UnboundedSender<crate::conversations::AgentEvent>,
    ) -> Result<Vec<Box<dyn InputHandler + Send>>> {
        let (permission_response_tx, _) = tokio::sync::mpsc::unbounded_channel();
        let (approval_response_tx, _) = tokio::sync::mpsc::unbounded_channel();

        let handlers: Vec<Box<dyn InputHandler + Send>> = vec![
            Box::new(handlers::PermissionHandler::new(
                permission_response_tx.clone(),
            )),
            Box::new(handlers::ApprovalHandler::new(approval_response_tx.clone())),
            Box::new(handlers::CompletionHandler::new()),
            Box::new(handlers::QuitHandler::new()),
            Box::new(handlers::SubmitHandler::new()),
            Box::new(handlers::PasteHandler::new()),
            Box::new(handlers::TextInputHandler::new()),
        ];

        Ok(handlers)
    }

    pub fn setup_summarizer(&self) -> Result<Arc<MessageSummarizer>> {
        let summarizer = Arc::new(MessageSummarizer::new(Arc::clone(&self.backend)));
        Ok(summarizer)
    }

    pub fn create_event_loop_context(
        &self,
        event_rx: mpsc::UnboundedReceiver<crate::conversations::AgentEvent>,
        event_tx: mpsc::UnboundedSender<crate::conversations::AgentEvent>,
        agent_manager: Arc<AgentManager>,
        conversation: Arc<tokio::sync::Mutex<Conversation>>,
        tool_executor: Arc<ToolExecutor>,
        input_handlers: Vec<Box<dyn InputHandler + Send>>,
        permission_manager: Arc<PermissionManager>,
        summarizer: Arc<MessageSummarizer>,
        default_agent: Option<Agent>,
    ) -> Result<EventLoopContext> {
        let mut command_registry = CommandRegistry::new();
        register_default_commands(&mut command_registry)
            .context("Failed to register default commands")?;
        let command_registry = Arc::new(command_registry);

        let current_agent_name = default_agent
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "assistant".to_string());

        Ok(EventLoopContext {
            backend: Arc::clone(&self.backend),
            parser: Arc::clone(&self.parser),
            tool_registry: Arc::new(self.tool_registry.clone()),
            tool_executor,
            conversation,
            event_rx,
            event_tx,
            command_registry,
            agent_manager,
            working_dir: self.working_dir_display.clone(),
            permission_manager,
            summarizer,
            input_handlers,
            current_agent_name,
            config: self.config.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bootstrap() -> TuiBootstrap {
        use crate::backends::mock::MockBackend;

        let backend = Box::new(MockBackend::new());
        let parser = MessageParser::new();
        let permission_manager = PermissionManager::new();
        let tool_registry = ToolRegistry::new();
        let config = AppConfig::default();

        TuiBootstrap::new(backend, parser, permission_manager, tool_registry, config)
    }

    #[test]
    fn test_bootstrap_creation() {
        let bootstrap = create_test_bootstrap();
        assert!(!bootstrap.working_dir_display.is_empty());
    }

    #[tokio::test]
    async fn test_setup_agents() {
        let bootstrap = create_test_bootstrap();
        let result = bootstrap.setup_agents();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_setup_channels() {
        let bootstrap = create_test_bootstrap();
        let result = bootstrap.setup_channels();
        assert!(result.is_ok());
        let (_tx, _rx, _pm) = result.unwrap();
    }

    #[tokio::test]
    async fn test_setup_input_handlers() {
        let bootstrap = create_test_bootstrap();
        let (_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let result = bootstrap.setup_input_handlers(&_tx);
        assert!(result.is_ok());
        let handlers = result.unwrap();
        assert_eq!(handlers.len(), 7);
    }

    #[tokio::test]
    async fn test_setup_summarizer() {
        let bootstrap = create_test_bootstrap();
        let result = bootstrap.setup_summarizer();
        assert!(result.is_ok());
    }
}
