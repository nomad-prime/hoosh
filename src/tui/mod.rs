mod app;
mod events;
mod terminal;
mod ui;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::conversations::{Conversation, ConversationEvent, ConversationHandler};
use crate::parser::MessageParser;
use crate::permissions::PermissionManager;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

use app::AppState;
use events::AgentEvent;
use terminal::{init_terminal, restore_terminal};

pub async fn run(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
) -> Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = AppState::new();

    app.add_message(format!("🚀 Welcome to hoosh! Using backend: {}", backend.backend_name()));
    app.add_message("📁 File system integration enabled - use @filename to reference files".to_string());

    if !permission_manager.is_enforcing() {
        app.add_message("⚠️ Permission checks disabled (--skip-permissions)".to_string());
    }

    let agent_manager = AgentManager::new()?;
    let default_agent = agent_manager.get_default_agent();

    if let Some(ref agent) = default_agent {
        app.add_message(format!("📝 Agent: {}", agent.name));
    } else {
        app.add_message("⚠️ No agent loaded".to_string());
    }

    app.add_message("Type your message and press Enter to send. Ctrl+C to quit.".to_string());
    app.add_message(String::new());

    let conversation = Arc::new(tokio::sync::Mutex::new({
        let mut conv = Conversation::new();
        if let Some(agent) = default_agent {
            conv.add_system_message(agent.content);
        }
        conv
    }));

    let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let result = run_event_loop(
        &mut terminal,
        &mut app,
        backend,
        parser,
        tool_registry,
        tool_executor,
        conversation,
        &mut event_rx,
        event_tx,
    )
    .await;

    restore_terminal(terminal)?;
    result
}

async fn run_event_loop(
    terminal: &mut terminal::Tui,
    app: &mut AppState,
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    tool_registry: ToolRegistry,
    tool_executor: ToolExecutor,
    conversation: Arc<tokio::sync::Mutex<Conversation>>,
    event_rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
    event_tx: mpsc::UnboundedSender<ConversationEvent>,
) -> Result<()> {
    let backend = Arc::new(backend);
    let parser = Arc::new(parser);
    let tool_registry = Arc::new(tool_registry);
    let tool_executor = Arc::new(tool_executor);

    let mut agent_task: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        terminal.draw(|f| ui::render(f, app))?;

        // Check for agent events
        while let Ok(event) = event_rx.try_recv() {
            let agent_event = match event {
                ConversationEvent::Thinking => AgentEvent::Thinking,
                ConversationEvent::AssistantThought(content) => {
                    AgentEvent::AssistantThought(content)
                }
                ConversationEvent::ToolCalls(calls) => {
                    AgentEvent::ToolCalls(calls)
                }
                ConversationEvent::ToolResult { tool_name, summary } => {
                    AgentEvent::ToolResult { tool_name, summary }
                }
                ConversationEvent::FinalResponse(content) => {
                    AgentEvent::FinalResponse(content)
                }
                ConversationEvent::Error(error) => {
                    AgentEvent::Error(error)
                }
                ConversationEvent::MaxStepsReached(max_steps) => {
                    AgentEvent::MaxStepsReached(max_steps)
                }
            };
            app.handle_agent_event(agent_event);
        }

        // Check if agent task is done
        if let Some(task) = &agent_task {
            if task.is_finished() {
                agent_task = None;
            }
        }

        // Poll for keyboard events with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Enter => {
                        let input_text = app.get_input_text();
                        if !input_text.trim().is_empty() && agent_task.is_none() {
                            app.add_message(format!("> {}", input_text));
                            app.clear_input();

                            let parser = Arc::clone(&parser);
                            let conversation = Arc::clone(&conversation);
                            let backend = Arc::clone(&backend);
                            let tool_registry = Arc::clone(&tool_registry);
                            let tool_executor = Arc::clone(&tool_executor);
                            let event_tx_clone = event_tx.clone();

                            agent_task = Some(tokio::spawn(async move {
                                let expanded_input = match parser.expand_message(&input_text).await {
                                    Ok(expanded) => expanded,
                                    Err(_) => input_text,
                                };

                                {
                                    let mut conv = conversation.lock().await;
                                    conv.add_user_message(expanded_input);
                                }

                                let mut conv = conversation.lock().await;
                                let handler = ConversationHandler::new(
                                    &backend,
                                    &tool_registry,
                                    &tool_executor,
                                )
                                .with_event_sender(event_tx_clone);

                                if let Err(e) = handler.handle_turn(&mut conv).await {
                                    eprintln!("Error handling turn: {}", e);
                                }
                            }));
                        }
                    }
                    _ => {
                        app.input.input(key);
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    if let Some(task) = agent_task {
        let _ = task.await;
    }

    Ok(())
}
