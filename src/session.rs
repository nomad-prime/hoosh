use anyhow::Result;
use chrono::Local;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::Conversation;
use crate::agent_definition::AgentDefinitionManager;
use crate::backends::LlmBackend;
use crate::commands::{register_default_commands, CommandRegistry};
use crate::completion::{CommandCompleter, FileCompleter};
use crate::config::AppConfig;
use crate::context_management::{
    ContextManager, MessageSummarizer, SlidingWindowStrategy, ToolOutputTruncationStrategy,
};
use crate::history::PromptHistory;
use crate::parser::MessageParser;
use crate::permissions::PermissionManager;
use crate::storage::ConversationStorage;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;
use crate::tui::app_loop::{
    ConversationState, EventChannels, EventLoopContext, RuntimeState, SystemResources,
};
use crate::tui::app_state::AppState;
use crate::tui::handlers;
use crate::tui::header;
use crate::tui::input_handler::InputHandler;
use crate::{SubAgentToolProvider, TaskToolProvider};

/// Represents the fully initialized session resources needed to run the agent
pub struct AgentSession {
    pub app_state: AppState,
    pub event_loop_context: EventLoopContext,
}

/// Parameters needed to initialize an agent session
pub struct SessionConfig {
    pub backend: Arc<dyn LlmBackend>,
    pub parser: MessageParser,
    pub skip_permissions: bool,
    pub tool_registry: ToolRegistry,
    pub config: AppConfig,
    pub continue_conversation_id: Option<String>,
    pub working_dir: PathBuf,
}

impl SessionConfig {
    pub fn new(
        backend: Arc<dyn LlmBackend>,
        parser: MessageParser,
        skip_permissions: bool,
        tool_registry: ToolRegistry,
        config: AppConfig,
        continue_conversation_id: Option<String>,
    ) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            backend,
            parser,
            skip_permissions,
            tool_registry,
            config,
            continue_conversation_id,
            working_dir,
        }
    }

    pub fn with_working_dir(mut self, working_dir: PathBuf) -> Self {
        self.working_dir = working_dir;
        self
    }
}

/// Initialize a complete agent session with all required resources
pub async fn initialize_session(session_config: SessionConfig) -> Result<AgentSession> {
    let SessionConfig {
        backend,
        parser,
        skip_permissions,
        mut tool_registry,
        config,
        continue_conversation_id,
        working_dir,
    } = session_config;

    // Initialize app state with history
    let mut app_state = AppState::new();
    load_history(&mut app_state);

    // Setup completers
    setup_completers(&mut app_state, &working_dir).await?;

    // Setup agent manager
    let agent_manager = Arc::new(AgentDefinitionManager::new()?);
    let default_agent = agent_manager.get_default_agent();

    // Display header
    let working_dir_display = working_dir
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| ".".to_string());

    let agent_name = default_agent.as_ref().map(|a| a.name.as_str());
    for line in header::create_header_block(
        backend.backend_name(),
        backend.model_name(),
        &working_dir_display,
        agent_name,
        None,
    ) {
        app_state.add_styled_line(line);
    }

    // Setup channels
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (permission_response_tx, permission_response_rx) = mpsc::unbounded_channel();
    let (approval_response_tx, approval_response_rx) = mpsc::unbounded_channel();

    // Setup permission manager
    let permission_manager = setup_permission_manager(
        event_tx.clone(),
        permission_response_rx,
        skip_permissions,
        &working_dir,
        &mut app_state,
    )?;

    let subagent_tool_registry = Arc::new(
        ToolRegistry::new().with_provider(Arc::new(SubAgentToolProvider::new(working_dir.clone()))),
    );

    tool_registry.add_provider(Arc::new(TaskToolProvider::new(
        Arc::clone(&backend),
        Arc::clone(&subagent_tool_registry),
        Arc::clone(&permission_manager),
    )));

    let tool_registry = Arc::new(tool_registry);

    // Setup conversation storage and load conversation
    let conversation_storage = Arc::new(ConversationStorage::with_default_path()?);
    let conversation_id = setup_conversation(
        &conversation_storage,
        continue_conversation_id,
        &mut app_state,
    )?;

    // Initialize conversation
    let conversation = load_or_create_conversation(
        Arc::clone(&conversation_storage),
        &conversation_id,
        default_agent.as_ref(),
        &backend,
        &working_dir,
    )?;
    let conversation = Arc::new(tokio::sync::Mutex::new(conversation));

    app_state.add_message("\n".to_string());

    // Setup tool execution
    let tool_executor =
        ToolExecutor::new(Arc::clone(&tool_registry), Arc::clone(&permission_manager))
            .with_event_sender(event_tx.clone())
            .with_autopilot_state(Arc::clone(&app_state.autopilot_enabled))
            .with_approval_receiver(approval_response_rx);

    // Setup input handlers
    let input_handlers = create_input_handlers(permission_response_tx, approval_response_tx);

    // Setup context management
    let summarizer = Arc::new(MessageSummarizer::new(Arc::clone(&backend)));
    let context_manager = setup_context_manager(&config);

    let command_registry = setup_command_registry()?;

    // Register command completer after session is initialized
    let command_completer = CommandCompleter::new(Arc::clone(&command_registry));
    app_state.register_completer(Box::new(command_completer));

    // Build system resources
    let system_resources = SystemResources {
        backend,
        parser: Arc::new(parser),
        tool_registry: Arc::clone(&tool_registry),
        tool_executor: Arc::new(tool_executor),
        agent_manager,
        command_registry,
    };

    // Build conversation state
    let conversation_state = ConversationState {
        conversation,
        summarizer,
        context_manager,
        current_agent_name: default_agent
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "assistant".to_string()),
        conversation_storage,
        conversation_id,
    };

    // Build event channels
    let channels = EventChannels { event_rx, event_tx };

    // Build runtime state
    let runtime = RuntimeState {
        permission_manager: Arc::clone(&permission_manager),
        input_handlers,
        working_dir: working_dir_display,
        config,
    };

    let event_loop_context = EventLoopContext {
        system_resources,
        conversation_state,
        channels,
        runtime,
    };

    Ok(AgentSession {
        app_state,
        event_loop_context,
    })
}

fn load_history(app_state: &mut AppState) {
    if let Some(history_path) = PromptHistory::default_history_path()
        && let Ok(history) = PromptHistory::with_file(1000, &history_path)
    {
        app_state.prompt_history = history;
    }
}

async fn setup_completers(app_state: &mut AppState, working_dir: &Path) -> Result<()> {
    let file_completer = FileCompleter::new(working_dir.to_path_buf());
    app_state.register_completer(Box::new(file_completer));
    Ok(())
}

fn setup_command_registry() -> Result<Arc<CommandRegistry>> {
    let mut command_registry = CommandRegistry::new();
    register_default_commands(&mut command_registry)?;
    let command_registry = Arc::new(command_registry);
    Ok(command_registry)
}

fn setup_permission_manager(
    event_tx: mpsc::UnboundedSender<crate::agent::AgentEvent>,
    permission_response_rx: mpsc::UnboundedReceiver<crate::agent::PermissionResponse>,
    skip_permissions: bool,
    working_dir: &Path,
    app_state: &mut AppState,
) -> Result<Arc<PermissionManager>> {
    let permission_manager = PermissionManager::new(event_tx, permission_response_rx)
        .with_skip_permissions(skip_permissions)
        .with_project_root(working_dir.to_path_buf())
        .inspect_err(|e| {
            use crate::console::console;
            console().error(&e.to_string());
        })?;

    if !permission_manager.is_enforcing() {
        app_state.add_message("⚠️ Permission checks disabled (--skip-permissions)".to_string());
    }

    Ok(Arc::new(permission_manager))
}

fn setup_conversation(
    conversation_storage: &ConversationStorage,
    continue_conversation_id: Option<String>,
    app_state: &mut AppState,
) -> Result<String> {
    if let Some(ref conv_id) = continue_conversation_id {
        if !conversation_storage.conversation_exists(conv_id) {
            use crate::console::console;
            console().error(&format!("Conversation '{}' not found", conv_id));
            std::process::exit(1);
        }

        let metadata = conversation_storage.load_metadata(conv_id).map_err(|e| {
            use crate::console::console;
            console().error(&format!("Failed to load conversation metadata: {}", e));
            e
        })?;

        if !metadata.title.is_empty() {
            app_state.add_message(format!("Continuing: {}", metadata.title));
        }

        Ok(conv_id.clone())
    } else {
        // Just generate ID - conversation will be created in load_or_create_conversation
        // with the system message included
        let conv_id = ConversationStorage::generate_conversation_id();
        Ok(conv_id)
    }
}

fn get_git_status(working_dir: &Path) -> String {
    match Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(working_dir)
        .output()
    {
        Ok(output) if output.status.success() => {
            let status = String::from_utf8_lossy(&output.stdout);
            if status.trim().is_empty() {
                "No uncommitted changes".to_string()
            } else {
                format!("Modifications:\n{}", status.trim())
            }
        }
        _ => "Git status unavailable".to_string(),
    }
}

fn get_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

fn generate_environment_context(backend: &Arc<dyn LlmBackend>, working_dir: &Path) -> Result<String> {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let platform = get_platform();
    let pwd = working_dir
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| ".".to_string());
    let git_status = get_git_status(working_dir);
    let model_info = format!(
        "{} ({})",
        backend.model_name(),
        backend.backend_name()
    );

    let context = format!(
        r#"
**Environment Context**

- **Working Directory**: {}
- **Date**: {}
- **Platform**: {}
- **Model**: {}

**Git Status**:
{}
"#,
        pwd, date, platform, model_info, git_status
    );

    Ok(context)
}

fn load_or_create_conversation(
    conversation_storage: Arc<ConversationStorage>,
    conversation_id: &str,
    default_agent: Option<&crate::agent_definition::AgentDefinition>,
    backend: &Arc<dyn LlmBackend>,
    working_dir: &Path,
) -> Result<Conversation> {
    // Try to load existing conversation
    if conversation_storage.conversation_exists(conversation_id) {
        match Conversation::load(conversation_id, conversation_storage) {
            Ok(conv) => return Ok(conv),
            Err(e) => {
                use crate::console::console;
                console().error(&format!("Failed to load conversation: {}", e));
                return Err(e);
            }
        }
    }

    // Create new conversation with storage
    let mut conv = Conversation::with_storage(conversation_id.to_string(), conversation_storage)?;
    if let Some(agent) = default_agent {
        conv.add_system_message(agent.content.clone());
    }

    // Add environment context system prompt for new conversations
    let env_context = generate_environment_context(backend, working_dir)?;
    conv.add_system_message(env_context);

    Ok(conv)
}

fn create_input_handlers(
    permission_response_tx: mpsc::UnboundedSender<crate::agent::PermissionResponse>,
    approval_response_tx: mpsc::UnboundedSender<crate::agent::ApprovalResponse>,
) -> Vec<Box<dyn InputHandler + Send>> {
    vec![
        Box::new(handlers::PermissionHandler::new(permission_response_tx)),
        Box::new(handlers::ApprovalHandler::new(approval_response_tx)),
        Box::new(handlers::CompletionHandler::new()),
        Box::new(handlers::QuitHandler::new()),
        Box::new(handlers::SubmitHandler::new()),
        Box::new(handlers::PasteHandler::new()),
        Box::new(handlers::TextInputHandler::new()),
    ]
}

fn setup_context_manager(config: &AppConfig) -> Arc<ContextManager> {
    let context_manager_config = config.get_context_manager_config();
    let token_accountant = Arc::new(crate::context_management::TokenAccountant::new());

    let mut context_manager_builder = ContextManager::new(
        context_manager_config.clone(),
        Arc::clone(&token_accountant),
    );

    // Apply sliding window FIRST to remove old messages
    if let Some(sliding_window_config) = context_manager_config.sliding_window {
        let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
        context_manager_builder =
            context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
    }

    // Apply truncation SECOND to reduce size of remaining messages
    if let Some(truncation_config) = context_manager_config.tool_output_truncation {
        let truncation_strategy = ToolOutputTruncationStrategy::new(truncation_config);
        context_manager_builder =
            context_manager_builder.add_strategy(Box::new(truncation_strategy));
    }

    Arc::new(context_manager_builder)
}
