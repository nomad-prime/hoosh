mod app;
mod events;
mod header;
mod terminal;
mod ui;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
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

    let agent_manager = AgentManager::new()?;
    let default_agent = agent_manager.get_default_agent();

    // Add ASCII art header with backend and agent info
    let agent_name = default_agent.as_ref().map(|a| a.name.as_str());
    for line in header::create_header_block(backend.backend_name(), agent_name) {
        app.add_styled_line(line);
    }

    app.add_message(
        "📁 File system integration enabled - use @filename to reference files".to_string(),
    );

    if !permission_manager.is_enforcing() {
        app.add_message("⚠️ Permission checks disabled (--skip-permissions)".to_string());
    }

    app.add_message("Type your message and press Enter to send.".to_string());
    app.add_message(
        "Keybindings: Ctrl+C (quit) | Ctrl+↑/↓ (scroll) | PageUp/Down (fast scroll)".to_string(),
    );
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
                ConversationEvent::ToolCalls(calls) => AgentEvent::ToolCalls(calls),
                ConversationEvent::ToolResult { tool_name, summary } => {
                    AgentEvent::ToolResult { tool_name, summary }
                }
                ConversationEvent::ToolExecutionComplete => AgentEvent::ToolExecutionComplete,
                ConversationEvent::FinalResponse(content) => AgentEvent::FinalResponse(content),
                ConversationEvent::Error(error) => AgentEvent::Error(error),
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

        // Poll for keyboard and mouse events with timeout
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::PageUp => {
                        for _ in 0..10 {
                            app.scroll_up();
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..10 {
                            app.scroll_down();
                        }
                    }
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.scroll_up();
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.scroll_down();
                    }
                    KeyCode::Enter => {
                        let input_text = app.get_input_text();
                        if !input_text.trim().is_empty() && agent_task.is_none() {
                            app.add_message(format!("> {}", input_text));
                            app.add_message("\n".to_string());
                            app.clear_input();

                            let parser = Arc::clone(&parser);
                            let conversation = Arc::clone(&conversation);
                            let backend = Arc::clone(&backend);
                            let tool_registry = Arc::clone(&tool_registry);
                            let tool_executor = Arc::clone(&tool_executor);
                            let event_tx_clone = event_tx.clone();

                            agent_task = Some(tokio::spawn(async move {
                                let expanded_input = match parser.expand_message(&input_text).await
                                {
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
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        app.scroll_down();
                    }
                    MouseEventKind::ScrollDown => {
                        app.scroll_up();
                    }
                    _ => {}
                },
                _ => {}
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
