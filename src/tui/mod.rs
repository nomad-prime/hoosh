mod actions;
mod app;
mod app_layout;
mod app_layout_builder;
mod clipboard;
pub mod components;
mod event_loop;
mod events;
mod handler_result;
pub mod handlers;
mod header;
mod input_handler;
mod layout_builder;
mod message_renderer;
mod terminal;

pub use message_renderer::MessageRenderer;

use anyhow::Result;
use std::sync::Arc;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::{CommandRegistry, register_default_commands};
use crate::config::AppConfig;
use crate::conversations::{ContextManager, MessageSummarizer};
use crate::parser::MessageParser;
use crate::permissions::PermissionManager;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

use crate::completion::{CommandCompleter, FileCompleter};
use crate::history::PromptHistory;
use crate::tui::terminal::{init_terminal, restore_terminal};
use app::AppState;
use event_loop::{
    ConversationState, EventChannels, EventLoopContext, RuntimeState, SystemResources,
    run_event_loop,
};

pub async fn run(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
    config: AppConfig,
) -> Result<()> {
    let terminal = init_terminal()?;
    let mut app = AppState::new();

    // Load history from file
    if let Some(history_path) = PromptHistory::default_history_path()
        && let Ok(history) = PromptHistory::with_file(1000, &history_path)
    {
        app.prompt_history = history;
    }

    // Setup working directory
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let working_dir_display = working_dir
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| ".".to_string());

    // Register completers
    let file_completer = FileCompleter::new(working_dir.clone());
    app.register_completer(Box::new(file_completer));

    let mut command_registry = CommandRegistry::new();
    register_default_commands(&mut command_registry)?;
    let command_registry = Arc::new(command_registry);
    let command_completer = CommandCompleter::new(Arc::clone(&command_registry));
    app.register_completer(Box::new(command_completer));

    // Setup agent manager
    let agent_manager = AgentManager::new()?;
    let agent_manager = Arc::new(agent_manager);
    let default_agent = agent_manager.get_default_agent();

    // Wrap backend in Arc for shared ownership
    let backend: Arc<dyn LlmBackend> = Arc::from(backend);

    // Add header
    let agent_name = default_agent.as_ref().map(|a| a.name.as_str());
    for line in header::create_header_block(
        backend.backend_name(),
        backend.model_name(),
        &working_dir_display,
        agent_name,
        None, // Initially no project is trusted
    ) {
        app.add_styled_line(line);
    }

    if !permission_manager.is_enforcing() {
        app.add_message("⚠️ Permission checks disabled (--skip-permissions)".to_string());
    }

    app.add_message("\n".to_string());

    // Setup conversation
    let conversation = Arc::new(tokio::sync::Mutex::new({
        let mut conv = crate::conversations::Conversation::new();
        if let Some(ref agent) = default_agent {
            conv.add_system_message(agent.content.clone());
        }
        conv
    }));

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (permission_response_tx, permission_response_rx) = tokio::sync::mpsc::unbounded_channel();
    let (approval_response_tx, approval_response_rx) = tokio::sync::mpsc::unbounded_channel();

    // Recreate permission manager with the TUI channels
    let skip_perms = permission_manager.skip_permissions();
    let default_perm = permission_manager.default_permission();
    let permission_manager = PermissionManager::new(event_tx.clone(), permission_response_rx)
        .with_skip_permissions(skip_perms)
        .with_default_permission(default_perm);

    let permission_manager_arc = Arc::new(permission_manager.clone());

    let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager)
        .with_event_sender(event_tx.clone())
        .with_autopilot_state(std::sync::Arc::clone(&app.autopilot_enabled)) // Share autopilot state
        .with_approval_receiver(approval_response_rx);

    let input_handlers: Vec<Box<dyn input_handler::InputHandler + Send>> = vec![
        // High priority: dialogs
        Box::new(handlers::PermissionHandler::new(
            permission_response_tx.clone(),
        )),
        Box::new(handlers::ApprovalHandler::new(approval_response_tx.clone())),
        Box::new(handlers::CompletionHandler::new()),
        // Medium priority: special keys
        Box::new(handlers::QuitHandler::new()),
        Box::new(handlers::SubmitHandler::new()),
        // Low priority: paste and text input (fallbacks)
        Box::new(handlers::PasteHandler::new()),
        Box::new(handlers::TextInputHandler::new()),
    ];

    let summarizer = Arc::new(MessageSummarizer::new(Arc::clone(&backend)));

    let context_manager_config = config.get_context_manager_config();
    let token_accountant = Arc::new(crate::conversations::TokenAccountant::new());
    let context_manager = Arc::new(ContextManager::new(
        context_manager_config,
        Arc::clone(&summarizer),
        token_accountant,
    ));

    let context = EventLoopContext {
        system_resources: SystemResources {
            backend,
            parser: Arc::new(parser),
            tool_registry: Arc::new(tool_registry),
            tool_executor: Arc::new(tool_executor),
            agent_manager,
            command_registry,
        },
        conversation_state: ConversationState {
            conversation,
            summarizer,
            context_manager,
            current_agent_name: default_agent
                .as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "assistant".to_string()),
        },
        channels: EventChannels { event_rx, event_tx },
        runtime: RuntimeState {
            permission_manager: permission_manager_arc,
            input_handlers,
            working_dir: working_dir_display,
            config,
        },
    };

    let terminal = run_event_loop(terminal, &mut app, context).await?;

    let _ = app.prompt_history.save();

    restore_terminal(terminal)?;
    Ok(())
}
