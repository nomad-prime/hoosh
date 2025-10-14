mod app;
mod completion;
mod events;
mod header;
mod terminal;
mod ui;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::conversations::{AgentEvent, Conversation, ConversationHandler, PermissionResponse};
use crate::parser::MessageParser;
use crate::permissions::{PermissionManager, PermissionScope};
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

use app::AppState;
use completion::FileCompleter;
use terminal::{init_terminal, restore_terminal};

pub async fn run(
    backend: Box<dyn LlmBackend>,
    parser: MessageParser,
    permission_manager: PermissionManager,
    tool_registry: ToolRegistry,
) -> Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = AppState::new();

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let file_completer = FileCompleter::new(working_dir);
    app.register_completer(Box::new(file_completer));

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
    app.add_message("\n".to_string());
    app.add_message(String::new());

    let conversation = Arc::new(tokio::sync::Mutex::new({
        let mut conv = Conversation::new();
        if let Some(agent) = default_agent {
            conv.add_system_message(agent.content);
        }
        conv
    }));

    // Create event channels
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    // Create permission response channel
    let (permission_response_tx, permission_response_rx) = mpsc::unbounded_channel();

    // Configure permission manager with event channels
    let permission_manager = permission_manager
        .with_event_sender(event_tx.clone())
        .with_response_receiver(permission_response_rx);

    let tool_executor = ToolExecutor::new(tool_registry.clone(), permission_manager);

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
        permission_response_tx,
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
    event_rx: &mut mpsc::UnboundedReceiver<AgentEvent>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    permission_response_tx: mpsc::UnboundedSender<PermissionResponse>,
) -> Result<()> {
    let backend = Arc::new(backend);
    let parser = Arc::new(parser);
    let tool_registry = Arc::new(tool_registry);
    let tool_executor = Arc::new(tool_executor);

    let mut agent_task: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        // Insert pending messages above viewport using insert_before
        if app.has_pending_messages() {
            use app::MessageLine;
            use ratatui::text::{Line, Text};
            use ratatui::widgets::{Paragraph, Widget};

            for msg in app.drain_pending_messages() {
                match msg {
                    MessageLine::Plain(text) => {
                        // Split multi-line text into individual lines
                        let lines: Vec<Line> = if text.is_empty() {
                            vec![Line::from("")]
                        } else {
                            text.lines()
                                .map(|line| Line::from(line.to_string()))
                                .collect()
                        };

                        let line_count = lines.len() as u16;
                        terminal.insert_before(line_count, |buf| {
                            Paragraph::new(Text::from(lines)).render(buf.area, buf);
                        })?;
                    }
                    MessageLine::Styled(styled_line) => {
                        terminal.insert_before(1, |buf| {
                            Paragraph::new(styled_line).render(buf.area, buf);
                        })?;
                    }
                }
            }
        }

        terminal.draw(|f| ui::render(f, app))?;

        // Check for agent events
        while let Ok(event) = event_rx.try_recv() {
            match event {
                AgentEvent::PermissionRequest { operation, request_id } => {
                    app.show_permission_dialog(operation, request_id);
                }
                other_event => {
                    app.handle_agent_event(other_event);
                }
            }
        }

        // Check if agent task is done
        if let Some(task) = &agent_task {
            if task.is_finished() {
                agent_task = None;
            }
        }

        // Tick animation frame
        app.tick_animation();

        // Poll for keyboard and mouse events with timeout
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle permission dialog keys first
                    if app.is_showing_permission_dialog() {
                        if let Some(dialog_state) = &app.permission_dialog_state {
                            let operation = dialog_state.operation.clone();
                            let request_id = dialog_state.request_id.clone();
                            let selected_option = dialog_state.options.get(dialog_state.selected_index).cloned();

                            let response = match key.code {
                                KeyCode::Up => {
                                    app.select_prev_permission_option();
                                    None
                                }
                                KeyCode::Down => {
                                    app.select_next_permission_option();
                                    None
                                }
                                KeyCode::Enter => {
                                    // Use the currently selected option
                                    selected_option.as_ref().and_then(|opt| {
                                        match opt {
                                            app::PermissionOption::YesOnce => Some((true, None)),
                                            app::PermissionOption::No => Some((false, None)),
                                            app::PermissionOption::AlwaysForFile => {
                                                let target = operation.target().to_string();
                                                Some((true, Some(PermissionScope::Specific(target))))
                                            }
                                            app::PermissionOption::AlwaysForDirectory(dir) => {
                                                Some((true, Some(PermissionScope::Directory(dir.clone()))))
                                            }
                                            app::PermissionOption::AlwaysForType => {
                                                Some((true, Some(PermissionScope::Global)))
                                            }
                                        }
                                    })
                                }
                                KeyCode::Char('y') | KeyCode::Char('Y') => Some((true, None)),
                                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                    Some((false, None))
                                }
                                KeyCode::Char('a') => {
                                    let target = operation.target().to_string();
                                    Some((true, Some(PermissionScope::Specific(target))))
                                }
                                KeyCode::Char('d') | KeyCode::Char('D') => {
                                    if let Some(dir) = operation.parent_directory() {
                                        Some((true, Some(PermissionScope::Directory(dir))))
                                    } else {
                                        let target = operation.target().to_string();
                                        Some((true, Some(PermissionScope::Specific(target))))
                                    }
                                }
                                KeyCode::Char('A') => Some((true, Some(PermissionScope::Global))),
                                _ => None,
                            };

                            if let Some((allowed, scope)) = response {
                                // Send permission response
                                let perm_response = PermissionResponse {
                                    request_id,
                                    allowed,
                                    scope,
                                };
                                let _ = permission_response_tx.send(perm_response);
                                app.hide_permission_dialog();
                            }
                        }
                        continue;
                    }

                    // Handle completion-specific keys first
                    if app.is_completing() {
                        match key.code {
                            KeyCode::Up => {
                                app.select_prev_completion();
                                continue;
                            }
                            KeyCode::Down => {
                                app.select_next_completion();
                                continue;
                            }
                            KeyCode::Tab | KeyCode::Enter => {
                                if let Some(selected) = app.apply_completion() {
                                    // Replace the query text with the selected completion
                                    let input_text = app.get_input_text();
                                    if let Some(at_pos) = input_text.rfind('@') {
                                        let new_text =
                                            format!("{}{}", &input_text[..=at_pos], selected);
                                        app.clear_input();
                                        for line in new_text.lines() {
                                            app.input.insert_str(line);
                                            if new_text.contains('\n') {
                                                app.input.insert_newline();
                                            }
                                        }
                                    }
                                }
                                // Don't fall through - always continue after applying completion
                                continue;
                            }
                            KeyCode::Esc => {
                                app.cancel_completion();
                                continue;
                            }
                            KeyCode::Backspace => {
                                app.input.input(key);
                                let input_text = app.get_input_text();
                                if let Some(at_pos) = input_text.rfind('@') {
                                    let query = input_text[at_pos + 1..].to_string();
                                    let completer_idx =
                                        app.completion_state.as_ref().map(|s| s.completer_index);

                                    if let Some(idx) = completer_idx {
                                        app.update_completion_query(query.clone());
                                        if let Some(completer) = app.completers.get(idx) {
                                            if let Ok(candidates) =
                                                completer.get_completions(&query).await
                                            {
                                                app.set_completion_candidates(candidates);
                                            }
                                        }
                                    }
                                } else {
                                    app.cancel_completion();
                                }
                                continue;
                            }
                            KeyCode::Char(_c) => {
                                app.input.input(key);
                                let input_text = app.get_input_text();
                                if let Some(at_pos) = input_text.rfind('@') {
                                    let query = input_text[at_pos + 1..].to_string();
                                    let completer_idx =
                                        app.completion_state.as_ref().map(|s| s.completer_index);

                                    if let Some(idx) = completer_idx {
                                        app.update_completion_query(query.clone());
                                        if let Some(completer) = app.completers.get(idx) {
                                            if let Ok(candidates) =
                                                completer.get_completions(&query).await
                                            {
                                                app.set_completion_candidates(candidates);
                                            }
                                        }
                                    }
                                }
                                continue;
                            }
                            _ => {
                                app.cancel_completion();
                            }
                        }
                    }

                    // Normal key handling
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true;
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
                                    let expanded_input =
                                        match parser.expand_message(&input_text).await {
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
                        KeyCode::Char(c) => {
                            // Check if this char triggers any completer
                            if let Some(completer_idx) = app.find_completer_for_key(c) {
                                app.input.input(key);
                                let cursor_pos = app.input.cursor();
                                app.start_completion(cursor_pos.0, completer_idx);

                                // Trigger initial completion with empty query
                                if let Some(completer) = app.completers.get(completer_idx) {
                                    if let Ok(candidates) = completer.get_completions("").await {
                                        app.set_completion_candidates(candidates);
                                    }
                                }
                            } else {
                                app.input.input(key);
                            }
                        }
                        _ => {
                            app.input.input(key);
                        }
                    }
                }
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
