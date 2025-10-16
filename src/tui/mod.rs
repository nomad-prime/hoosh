mod actions;
mod app;
pub mod completion;
mod event_loop;
mod events;
mod header;
pub mod history;
mod input_handlers;
mod terminal;
mod ui;

use anyhow::Result;
use std::sync::Arc;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::{register_default_commands, CommandRegistry};
use crate::parser::MessageParser;
use crate::permissions::PermissionManager;
use crate::tools::ToolRegistry;
use crate::tool_executor::ToolExecutor;

use app::AppState;
use completion::{CommandCompleter, FileCompleter};
use event_loop::{run_event_loop, EventLoopContext};
use history::PromptHistory;
use terminal::{init_terminal, restore_terminal};

pub async fn run(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
) -> Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = AppState::new();

    // Load history from file
    if let Some(history_path) = PromptHistory::default_history_path() {
        if let Ok(history) = PromptHistory::with_file(1000, &history_path) {
            app.prompt_history = history;
        }
    }

    // Setup working directory
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let working_dir_display = working_dir.to_str().unwrap_or(".").to_string();

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
        if let Some(agent) = default_agent {
            conv.add_system_message(agent.content);
        }
        conv
    }));

    // Create event channels
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (permission_response_tx, permission_response_rx) = tokio::sync::mpsc::unbounded_channel();
    let (approval_response_tx, approval_response_rx) = tokio::sync::mpsc::unbounded_channel();

    // Configure permission manager
    let permission_manager = permission_manager
        .with_event_sender(event_tx.clone())
        .with_response_receiver(permission_response_rx);

    let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager)
        .with_event_sender(event_tx.clone())
        .with_autopilot_state(std::sync::Arc::clone(&app.autopilot_enabled)) // Share autopilot state
        .with_approval_receiver(approval_response_rx);

    // Create context
    let context = EventLoopContext {
        backend,
        parser: Arc::new(parser),
        tool_registry: Arc::new(tool_registry),
        tool_executor: Arc::new(tool_executor),
        conversation,
        event_rx,
        event_tx,
        permission_response_tx,
        approval_response_tx,
        command_registry,
        agent_manager,
        working_dir: working_dir_display,
    };

    // Run event loop
    let result = run_event_loop(&mut terminal, &mut app, context).await;

    // Save history before exiting
    let _ = app.prompt_history.save();

    restore_terminal(terminal)?;
    result
}
