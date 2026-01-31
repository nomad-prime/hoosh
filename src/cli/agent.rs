use crate::backends::backend_factory::create_backend;
use crate::session::{SessionConfig, initialize_session};
use crate::terminal_mode::TerminalMode;
use crate::tools::todo_state::TodoState;
use crate::tui::init_permission;
use crate::tui::terminal::{init_terminal, restore_terminal};
use crate::{
    AppConfig, BuiltinToolProvider, ConversationStorage, LlmBackend, MessageParser, ToolRegistry,
    console,
};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn handle_agent(
    backend_name: Option<String>,
    add_dirs: Vec<String>,
    skip_permissions: bool,
    continue_last: bool,
    mode: Option<String>,
    message: Vec<String>,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let backend_name = backend_name.unwrap_or_else(|| config.default_backend.clone());

    let backend: Box<dyn LlmBackend> = create_backend(&backend_name, config)?;
    backend.initialize().await?;

    let working_dir = if !add_dirs.is_empty() {
        PathBuf::from(&add_dirs[0])
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let parser = MessageParser::with_working_directory(working_dir.clone());

    let backend_arc = Arc::from(backend);

    // Create shared todo state for the session
    let todo_state = TodoState::new();

    let tool_registry = ToolRegistry::new().with_provider(Arc::new(
        BuiltinToolProvider::with_todo_state(working_dir.clone(), todo_state.clone()),
    ));

    // Parse mode string to TerminalMode enum
    let terminal_mode = mode
        .as_deref()
        .and_then(|s| s.parse::<TerminalMode>().ok())
        .unwrap_or_default();

    // Handle permissions based on mode
    if !skip_permissions {
        match terminal_mode {
            TerminalMode::Tagged => {
                // Text-based permissions for tagged mode
                use crate::text_prompts;
                if let Err(e) =
                    text_prompts::handle_initial_permissions(&working_dir, &tool_registry)
                {
                    eprintln!("Permission setup failed: {}", e);
                    return Ok(());
                }
            }
            TerminalMode::Inline | TerminalMode::Fullview => {
                // TUI-based permissions for inline/fullview modes
                let terminal = init_terminal()?;
                let terminal = match init_permission::run(
                    terminal,
                    working_dir.clone(),
                    &tool_registry,
                    skip_permissions,
                )
                .await?
                {
                    (terminal, init_permission::InitialPermissionDialogResult::Cancelled) => {
                        restore_terminal(terminal)?;
                        return Ok(());
                    }
                    (
                        terminal,
                        init_permission::InitialPermissionDialogResult::SkippedPermissionsExist,
                    ) => terminal,
                    (terminal, init_permission::InitialPermissionDialogResult::Choice(_)) => {
                        terminal
                    }
                };
                restore_terminal(terminal)?;
                println!();
            }
        }
    }

    let continue_conversation_id = if continue_last {
        let storage = ConversationStorage::with_default_path()?;
        let conversations = storage.list_conversations()?;

        if let Some(latest) = conversations.first() {
            Some(latest.id.clone())
        } else {
            console().warning("No previous conversations found. Starting new conversation.");
            None
        }
    } else {
        None
    };

    // Initialize session with all resources
    let session_config = SessionConfig::new(
        Arc::clone(&backend_arc),
        parser,
        skip_permissions,
        tool_registry,
        config.clone(),
        continue_conversation_id,
        todo_state,
    )
    .with_working_dir(working_dir)
    .with_terminal_mode(Some(terminal_mode));

    let session = initialize_session(session_config).await?;

    // Prepare message for tagged mode (join all args into single string)
    let message_text = if !message.is_empty() {
        Some(message.join(" "))
    } else {
        None
    };

    // Single switch statement over TerminalMode - routes to appropriate mode implementation
    match session.terminal_mode {
        TerminalMode::Fullview => crate::tui::run_with_session_fullview(session).await?,
        TerminalMode::Inline => crate::tui::run_with_session_inline(session).await?,
        TerminalMode::Tagged => {
            let permission_response_tx = session
                .event_loop_context
                .tagged_mode_channels
                .permission_response_tx
                .clone();
            let approval_response_tx = session
                .event_loop_context
                .tagged_mode_channels
                .approval_response_tx
                .clone();
            crate::tagged_mode::run_tagged_mode(
                session,
                message_text,
                permission_response_tx,
                approval_response_tx,
            )
            .await?
        }
    }

    Ok(())
}
