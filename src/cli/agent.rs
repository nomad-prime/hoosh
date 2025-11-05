use crate::backends::backend_factory::create_backend;
use crate::{AppConfig, ConversationStorage, LlmBackend, MessageParser, ToolExecutor, console};
use std::path::PathBuf;

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

    let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(working_dir.clone());

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

    crate::tui::run_with_conversation(
        backend,
        parser,
        skip_permissions,
        tool_registry,
        config.clone(),
        continue_conversation_id,
    )
    .await?;

    Ok(())
}
