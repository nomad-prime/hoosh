use anyhow::Result;
use crossterm::event;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::CommandRegistry;
use crate::config::AppConfig;
use crate::conversations::{AgentEvent, Conversation, MessageSummarizer};
use crate::parser::MessageParser;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

use super::actions::{execute_command, start_agent_conversation};
use super::app::{AppState, MessageLine};
use super::input_handler::InputHandler;
use super::terminal::{resize_terminal, Tui};
use super::ui;

pub struct EventLoopContext {
    pub backend: Arc<dyn LlmBackend>,
    pub parser: Arc<MessageParser>,
    pub tool_registry: Arc<ToolRegistry>,
    pub tool_executor: Arc<ToolExecutor>,
    pub conversation: Arc<Mutex<Conversation>>,
    pub event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    pub event_tx: mpsc::UnboundedSender<AgentEvent>,
    pub command_registry: Arc<CommandRegistry>,
    pub agent_manager: Arc<AgentManager>,
    pub working_dir: String,
    pub permission_manager: Arc<crate::permissions::PermissionManager>,
    pub summarizer: Arc<MessageSummarizer>,
    pub input_handlers: Vec<Box<dyn InputHandler + Send>>,
    pub current_agent_name: String,
    pub config: AppConfig,
}

pub async fn run_event_loop(
    mut terminal: Tui,
    app: &mut AppState,
    mut context: EventLoopContext,
) -> Result<Tui> {
    let mut agent_task: Option<JoinHandle<()>> = None;
    let base_height = 6u16;
    let mut current_viewport_height = base_height;

    loop {
        // Insert pending messages above viewport
        if app.has_pending_messages() {
            for msg in app.drain_pending_messages() {
                match msg {
                    MessageLine::Plain(text) => {
                        // Pre-wrap text with indentation preserved for continuation lines
                        let terminal_width = terminal.size()?.width as usize;
                        let mut wrapped_lines = Vec::new();

                        if text.is_empty() {
                            wrapped_lines.push(Line::from(""));
                        } else {
                            for line in text.lines() {
                                if line.is_empty() {
                                    wrapped_lines.push(Line::from(""));
                                    continue;
                                }

                                // Detect leading whitespace for indentation
                                let indent_len = line.len() - line.trim_start().len();
                                let indent = " ".repeat(indent_len);

                                // Wrap with preserved indentation for continuation lines
                                let options = textwrap::Options::new(terminal_width)
                                    .initial_indent("")
                                    .subsequent_indent(&indent);

                                let wrapped = textwrap::wrap(line, &options);
                                for wrapped_line in wrapped {
                                    wrapped_lines.push(Line::from(wrapped_line.to_string()));
                                }
                            }
                        }

                        let line_count = wrapped_lines.len() as u16;
                        terminal.insert_before(line_count, |buf| {
                            Paragraph::new(Text::from(wrapped_lines)).render(buf.area, buf);
                        })?;
                    }
                    MessageLine::Styled(styled_line) => {
                        // For styled lines, we'll use ratatui's wrap since preserving
                        // style across wrapped lines is complex. Just calculate line count.
                        let terminal_width = terminal.size()?.width as usize;
                        let line_text: String = styled_line
                            .spans
                            .iter()
                            .map(|s| s.content.as_ref())
                            .collect();
                        let wrapped_line_count = if line_text.is_empty() {
                            1
                        } else {
                            textwrap::wrap(&line_text, terminal_width).len()
                        };

                        terminal.insert_before(wrapped_line_count as u16, |buf| {
                            Paragraph::new(styled_line)
                                .wrap(Wrap { trim: false })
                                .render(buf.area, buf);
                        })?;
                    }
                }
            }
        }

        // Dynamically adjust viewport height based on whether dialogs are shown
        let dialog_height = if app.is_showing_permission_dialog() {
            15 // Height needed for permission dialog
        } else if app.is_showing_approval_dialog() {
            10 // Height needed for approval dialog
        } else if app.is_completing() {
            12 // Height needed for completion popup
        } else {
            0 // No dialog, use base height
        };

        let desired_viewport_height = base_height + dialog_height;

        // Only recreate terminal if viewport height needs to change
        if desired_viewport_height != current_viewport_height {
            terminal = resize_terminal(terminal, desired_viewport_height)?;
            current_viewport_height = desired_viewport_height;
        }

        terminal.draw(|f| ui::render(f, app))?;

        // Check for agent events
        while let Ok(event) = context.event_rx.try_recv() {
            match event {
                AgentEvent::PermissionRequest {
                    operation,
                    request_id,
                } => {
                    app.show_permission_dialog(operation, request_id);
                }
                AgentEvent::ApprovalRequest {
                    tool_call_id,
                    tool_name,
                } => {
                    app.show_approval_dialog(tool_call_id, tool_name);
                }
                AgentEvent::Exit => {
                    app.should_quit = true;
                }
                AgentEvent::ClearConversation => {
                    let mut conv = context.conversation.lock().await;
                    conv.messages.clear();
                    app.add_message("Conversation cleared.\n".to_string());
                }
                AgentEvent::DebugMessage(_msg) => {
                    // Debug messages are currently suppressed
                    // app.add_message(format!("[DEBUG] {}\n", _msg));
                }
                AgentEvent::AgentSwitched { new_agent_name } => {
                    context.current_agent_name = new_agent_name;
                    // The header will be updated on next render
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

        // Poll for keyboard and mouse events
        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            let agent_task_active = agent_task.is_some();

            // Iterate through handlers in order until one handles the event
            for handler in &mut context.input_handlers {
                if !handler.should_handle(&event, app) {
                    continue;
                }

                match handler.handle_event(&event, app, agent_task_active).await {
                    Ok(super::handler_result::KeyHandlerResult::Handled) => {
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::ShouldQuit) => {
                        app.should_quit = true;
                        if let Some(task) = agent_task.take() {
                            task.abort();
                        }
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::ShouldCancelTask) => {
                        if let Some(task) = agent_task.take() {
                            task.abort();
                            app.agent_state = super::events::AgentState::Idle;
                            app.add_status_message(
                                "Task cancelled by user (press Ctrl+C again to quit)\n"
                            );
                        }
                        app.should_cancel_task = false;
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::StartCommand(input)) => {
                        use super::actions::CommandExecutionContext;
                        execute_command(CommandExecutionContext {
                            input,
                            command_registry: Arc::clone(&context.command_registry),
                            conversation: Arc::clone(&context.conversation),
                            tool_registry: Arc::clone(&context.tool_registry),
                            agent_manager: Arc::clone(&context.agent_manager),
                            working_dir: context.working_dir.clone(),
                            event_tx: context.event_tx.clone(),
                            permission_manager: Arc::clone(&context.permission_manager),
                            summarizer: Arc::clone(&context.summarizer),
                            current_agent_name: context.current_agent_name.clone(),
                            config: context.config.clone(),
                            backend: Arc::clone(&context.backend),
                        });
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::StartConversation(input)) => {
                        agent_task = Some(start_agent_conversation(
                            input,
                            Arc::clone(&context.parser),
                            Arc::clone(&context.conversation),
                            Arc::clone(&context.backend),
                            Arc::clone(&context.tool_registry),
                            Arc::clone(&context.tool_executor),
                            context.event_tx.clone(),
                        ));
                        break;
                    }
                    Err(_) => {
                        // Log error but continue to next handler
                        continue;
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Clean up any remaining agent task
    // (This should only happen if the loop exits without should_quit being set)
    if let Some(task) = agent_task {
        let _ = task.await;
    }

    Ok(terminal)
}
