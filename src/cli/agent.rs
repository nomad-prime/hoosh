use crate::backends::backend_factory::create_backend;
use crate::session::{SessionConfig, initialize_session};
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

    let tool_registry =
        ToolRegistry::new().with_provider(Arc::new(BuiltinToolProvider::new(working_dir.clone())));

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
        (terminal, init_permission::InitialPermissionDialogResult::SkippedPermissionsExist) => {
            terminal
        }
        (terminal, init_permission::InitialPermissionDialogResult::Choice(_)) => terminal,
    };
    restore_terminal(terminal)?;
    println!();

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
    )
    .with_working_dir(working_dir);

    let session = initialize_session(session_config).await?;

    // Run the TUI with initialized session
    crate::tui::run_with_session(session).await?;

    Ok(())
}
