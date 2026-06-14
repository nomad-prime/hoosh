// CLI handler for `hoosh shell` — builds an AgentSession (same path as the
// default agent command) and hands it to shell_mode::run_shell_mode.

use crate::backends::backend_factory::create_backend;
use crate::memory_mode::MemoryMode;
use crate::session::{SessionConfig, initialize_session};
use crate::shell_mode::run_shell_mode;
use crate::terminal_mode::TerminalMode;
use crate::tools::todo_state::TodoState;
use crate::{AppConfig, BuiltinToolProvider, LlmBackend, MessageParser, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn handle_shell(
    backend_name: Option<String>,
    skip_permissions: bool,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let backend_name = backend_name.unwrap_or_else(|| config.default_backend.clone());

    let backend: Box<dyn LlmBackend> = create_backend(&backend_name, config)?;
    backend.initialize().await?;
    let backend_arc = Arc::from(backend);

    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let parser = MessageParser::with_working_directory(working_dir.clone());

    let todo_state = TodoState::new();
    let tool_registry = ToolRegistry::new().with_provider(Arc::new(
        BuiltinToolProvider::with_todo_state(working_dir.clone(), todo_state.clone()),
    ));

    let session_config = SessionConfig::new(
        Arc::clone(&backend_arc),
        parser,
        skip_permissions,
        tool_registry,
        config.clone(),
        None,
        todo_state,
    )
    .with_working_dir(working_dir)
    .with_terminal_mode(Some(TerminalMode::Tagged))
    .with_memory_mode(MemoryMode::default());

    let session = initialize_session(session_config).await?;
    run_shell_mode(session).await
}
