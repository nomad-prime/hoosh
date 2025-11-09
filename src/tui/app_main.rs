use crate::completion::{CommandCompleter, FileCompleter};
use crate::context_management::{
    ContextCompressionStrategy, ContextManager, MessageSummarizer, SlidingWindowStrategy,
    ToolOutputTruncationStrategy,
};
use crate::history::PromptHistory;
use crate::tui::app_loop::{
    ConversationState, EventChannels, EventLoopContext, RuntimeState, SystemResources,
    run_event_loop,
};
use crate::tui::app_state::AppState;
use crate::tui::terminal::{init_terminal, restore_terminal};
use crate::tui::{handlers, header, init_permission_loop, input_handler};
use crate::{
    AgentDefinitionManager, AppConfig, CommandRegistry, ConversationStorage, LlmBackend,
    MessageParser, PermissionManager, ToolExecutor, ToolRegistry, register_default_commands,
};
use std::sync::Arc;

pub async fn run(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    skip_permissions: bool,
    tool_registry: ToolRegistry,
    config: AppConfig,
    continue_conversation_id: Option<String>,
) -> anyhow::Result<()> {
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
    let agent_manager = AgentDefinitionManager::new()?;
    let agent_manager = Arc::new(agent_manager);
    let default_agent = agent_manager.get_default_agent();

    use crate::permissions::storage::PermissionsFile;
    let permissions_path = PermissionsFile::get_permissions_path(&working_dir);
    let should_show_initial_dialog = !skip_permissions && !permissions_path.exists();

    let terminal = if should_show_initial_dialog {
        use crate::tui::app_state::InitialPermissionChoice;
        let (terminal, choice) =
            init_permission_loop::run(terminal, working_dir.clone(), &tool_registry).await?;

        if choice.is_none() || matches!(choice, Some(InitialPermissionChoice::Deny)) {
            restore_terminal(terminal)?;
            return Ok(());
        }

        // Clear the terminal to remove any remnants from the permission dialog
        let mut terminal = terminal;
        terminal.clear()?;
        terminal
    } else {
        terminal
    };

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (permission_response_tx, permission_response_rx) = tokio::sync::mpsc::unbounded_channel();
    let (approval_response_tx, approval_response_rx) = tokio::sync::mpsc::unbounded_channel();

    let permission_manager = match PermissionManager::new(event_tx.clone(), permission_response_rx)
        .with_skip_permissions(skip_permissions)
        .with_project_root(working_dir.clone())
    {
        Ok(pm) => pm,
        Err(err) => {
            use crate::console::console;
            console().error(&err.to_string());
            std::process::exit(1);
        }
    };

    let conversation_storage = match ConversationStorage::with_default_path() {
        Ok(storage) => Arc::new(storage),
        Err(e) => {
            use crate::console::console;
            console().error(&format!("Failed to initialize conversation storage: {}", e));
            std::process::exit(1);
        }
    };

    let (conversation_id, conversation_title) = if let Some(ref conv_id) = continue_conversation_id
    {
        if !conversation_storage.conversation_exists(conv_id) {
            use crate::console::console;
            console().error(&format!("Conversation '{}' not found", conv_id));
            std::process::exit(1);
        }

        let metadata = match conversation_storage.load_metadata(conv_id) {
            Ok(meta) => meta,
            Err(e) => {
                use crate::console::console;
                console().error(&format!("Failed to load conversation metadata: {}", e));
                std::process::exit(1);
            }
        };

        (conv_id.clone(), Some(metadata.title))
    } else {
        let conv_id = ConversationStorage::generate_conversation_id();
        if let Err(e) = conversation_storage.create_conversation(&conv_id) {
            use crate::console::console;
            console().error(&format!("Failed to create conversation: {}", e));
            std::process::exit(1);
        }
        (conv_id, None)
    };

    let backend: Arc<dyn LlmBackend> = Arc::from(backend);

    let agent_name = default_agent.as_ref().map(|a| a.name.as_str());
    for line in header::create_header_block(
        backend.backend_name(),
        backend.model_name(),
        &working_dir_display,
        agent_name,
        None,
    ) {
        app.add_styled_line(line);
    }

    if !permission_manager.is_enforcing() {
        app.add_message("⚠️ Permission checks disabled (--skip-permissions)".to_string());
    }

    if let Some(ref title) = conversation_title
        && !title.is_empty()
    {
        app.add_message(format!("Continuing: {}", title));
    }

    app.add_message("\n".to_string());

    let conversation = Arc::new(tokio::sync::Mutex::new({
        if let Some(ref conv_id_to_load) = continue_conversation_id {
            match conversation_storage.load_messages(conv_id_to_load) {
                Ok(messages) => {
                    let mut c = crate::agent::Conversation::new();
                    c.messages = messages;
                    c
                }
                Err(e) => {
                    use crate::console::console;
                    console().error(&format!("Failed to load conversation messages: {}", e));
                    std::process::exit(1);
                }
            }
        } else {
            let mut conv = crate::agent::Conversation::new();
            if let Some(ref agent) = default_agent {
                conv.add_system_message(agent.content.clone());
            }
            conv
        }
    }));

    let permission_manager_arc = Arc::new(permission_manager.clone());

    let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager)
        .with_event_sender(event_tx.clone())
        .with_autopilot_state(std::sync::Arc::clone(&app.autopilot_enabled))
        .with_approval_receiver(approval_response_rx);

    let input_handlers: Vec<Box<dyn input_handler::InputHandler + Send>> = vec![
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

    let summarizer = Arc::new(MessageSummarizer::new(Arc::clone(&backend)));

    let context_manager_config = config.get_context_manager_config();
    let token_accountant = Arc::new(crate::context_management::TokenAccountant::new());

    let compression_strategy = ContextCompressionStrategy::new(
        context_manager_config.clone(),
        Arc::clone(&summarizer),
        Arc::clone(&token_accountant),
    );

    let mut context_manager_builder = ContextManager::new(
        context_manager_config.clone(),
        Arc::clone(&token_accountant),
    );

    if let Some(truncation_config) = context_manager_config.tool_output_truncation {
        let truncation_strategy = ToolOutputTruncationStrategy::new(truncation_config);
        context_manager_builder =
            context_manager_builder.add_strategy(Box::new(truncation_strategy));
    }

    if let Some(sliding_window_config) = context_manager_config.sliding_window {
        let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
        context_manager_builder =
            context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
    }

    let context_manager =
        Arc::new(context_manager_builder.add_strategy(Box::new(compression_strategy)));

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
            conversation_storage,
            conversation_id,
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
