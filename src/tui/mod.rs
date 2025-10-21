mod actions;
mod app;
mod bootstrap;
mod clipboard;
pub mod completion;
pub mod components;
mod event_loop;
mod events;
mod handler_result;
pub mod handlers;
mod header;
pub mod history;
mod input_handler;
mod message_renderer;
mod terminal;
mod ui;
mod viewport_manager;

pub use message_renderer::MessageRenderer;

use anyhow::Result;

use crate::backends::LlmBackend;
use crate::config::AppConfig;
use crate::parser::MessageParser;
use crate::permissions::PermissionManager;
use crate::tools::ToolRegistry;

use bootstrap::TuiBootstrap;
use event_loop::run_event_loop;
use terminal::restore_terminal;

pub async fn run(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
    config: AppConfig,
) -> Result<()> {
    let bootstrap = TuiBootstrap::new(backend, parser, permission_manager, tool_registry, config);

    let (terminal, mut app) = bootstrap.init_terminal_and_app()?;
    bootstrap.setup_completers(&mut app)?;
    let agent_manager = bootstrap.setup_agents()?;
    let default_agent = agent_manager.get_default_agent();

    bootstrap.setup_header(&mut app, &default_agent)?;

    let conversation = bootstrap.setup_conversation(default_agent.as_ref())?;
    let (event_tx, event_rx, permission_manager_arc) = bootstrap.setup_channels()?;
    let tool_executor = bootstrap.setup_tool_executor(&app, &event_tx)?;
    let input_handlers = bootstrap.setup_input_handlers(&event_tx)?;
    let summarizer = bootstrap.setup_summarizer()?;

    let context = bootstrap.create_event_loop_context(
        event_rx,
        event_tx,
        agent_manager,
        conversation,
        tool_executor,
        input_handlers,
        permission_manager_arc,
        summarizer,
        default_agent,
    )?;

    let terminal = run_event_loop(terminal, &mut app, context).await?;

    let _ = app.prompt_history.save();

    restore_terminal(terminal)?;
    Ok(())
}
