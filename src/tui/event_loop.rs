use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Widget};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::CommandRegistry;
use crate::conversations::{AgentEvent, ApprovalResponse, Conversation, PermissionResponse};
use crate::parser::MessageParser;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

use super::actions::{execute_command, start_agent_conversation};
use super::app::{AppState, MessageLine};
use super::input_handlers::{
    handle_approval_keys, handle_completion_keys, handle_normal_keys, handle_permission_keys,
    KeyHandlerResult,
};
use super::terminal::Tui;
use super::ui;

pub struct EventLoopContext {
    pub backend: Arc<dyn LlmBackend>,
    pub parser: Arc<MessageParser>,
    pub tool_registry: Arc<ToolRegistry>,
    pub tool_executor: Arc<ToolExecutor>,
    pub conversation: Arc<Mutex<Conversation>>,
    pub event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    pub event_tx: mpsc::UnboundedSender<AgentEvent>,
    pub permission_response_tx: mpsc::UnboundedSender<PermissionResponse>,
    pub approval_response_tx: mpsc::UnboundedSender<ApprovalResponse>,
    pub command_registry: Arc<CommandRegistry>,
    pub agent_manager: Arc<AgentManager>,
    pub working_dir: String,
}

pub async fn run_event_loop(
    terminal: &mut Tui,
    app: &mut AppState,
    mut context: EventLoopContext,
) -> Result<()> {
    let mut agent_task: Option<JoinHandle<()>> = None;

    loop {
        // Insert pending messages above viewport
        if app.has_pending_messages() {
            for msg in app.drain_pending_messages() {
                match msg {
                    MessageLine::Plain(text) => {
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
            if let Event::Key(key) = event::read()? {
                // Try permission dialog handler first
                match handle_permission_keys(
                    key.code,
                    key.modifiers,
                    app,
                    &context.permission_response_tx,
                ) {
                    KeyHandlerResult::Handled => continue,
                    KeyHandlerResult::NotHandled => {}
                    KeyHandlerResult::ShouldQuit => {
                        app.should_quit = true;
                        // If quitting with an active agent task, abort it immediately
                        if let Some(task) = agent_task.take() {
                            task.abort();
                        }
                        break;
                    }
                    KeyHandlerResult::ShouldCancelTask => {
                        // Cancel the running task but don't quit
                        if let Some(task) = agent_task.take() {
                            task.abort();
                            app.agent_state = super::events::AgentState::Idle;
                            app.add_message(
                                " ⎿ Task cancelled by user (press Ctrl+C again to quit)\n"
                                    .to_string(),
                            );
                        }
                        app.should_cancel_task = false;
                        continue;
                    }
                    _ => {}
                }

                // Try approval dialog handler
                match handle_approval_keys(
                    key.code,
                    key.modifiers,
                    app,
                    &context.approval_response_tx,
                ) {
                    KeyHandlerResult::Handled => continue,
                    KeyHandlerResult::NotHandled => {}
                    KeyHandlerResult::ShouldQuit => {
                        app.should_quit = true;
                        // If quitting with an active agent task, abort it immediately
                        if let Some(task) = agent_task.take() {
                            task.abort();
                        }
                        break;
                    }
                    KeyHandlerResult::ShouldCancelTask => {
                        // Cancel the running task but don't quit
                        if let Some(task) = agent_task.take() {
                            task.abort();
                            app.agent_state = super::events::AgentState::Idle;
                            app.add_message(
                                " ⎿ Task cancelled by user (press Ctrl+C again to quit)\n"
                                    .to_string(),
                            );
                        }
                        app.should_cancel_task = false;
                        continue;
                    }
                    _ => {}
                }

                // Try completion handler
                match handle_completion_keys(key.code, app).await {
                    KeyHandlerResult::Handled => continue,
                    KeyHandlerResult::NotHandled => {}
                    KeyHandlerResult::ShouldQuit => {
                        app.should_quit = true;
                        // If quitting with an active agent task, abort it immediately
                        if let Some(task) = agent_task.take() {
                            task.abort();
                        }
                        break;
                    }
                    KeyHandlerResult::ShouldCancelTask => {
                        // Cancel the running task but don't quit
                        if let Some(task) = agent_task.take() {
                            task.abort();
                            app.agent_state = super::events::AgentState::Idle;
                            app.add_message(
                                " ⎿ Task cancelled by user (press Ctrl+C again to quit)\n"
                                    .to_string(),
                            );
                        }
                        app.should_cancel_task = false;
                        continue;
                    }
                    _ => {}
                }

                // Normal key handling
                let agent_task_active = agent_task.is_some();
                match handle_normal_keys(key.code, key.modifiers, app, agent_task_active).await {
                    KeyHandlerResult::ShouldQuit => {
                        app.should_quit = true;
                        // If quitting with an active agent task, abort it immediately
                        if let Some(task) = agent_task.take() {
                            task.abort();
                        }
                        break;
                    }
                    KeyHandlerResult::ShouldCancelTask => {
                        // Cancel the running task but don't quit
                        if let Some(task) = agent_task.take() {
                            task.abort();
                            app.agent_state = super::events::AgentState::Idle;
                            app.add_message(
                                " ⎿ Task cancelled by user (press Ctrl+C again to quit)\n"
                                    .to_string(),
                            );
                        }
                        app.should_cancel_task = false;
                        continue;
                    }
                    KeyHandlerResult::StartCommand(input) => {
                        execute_command(
                            input,
                            Arc::clone(&context.command_registry),
                            Arc::clone(&context.conversation),
                            Arc::clone(&context.tool_registry),
                            Arc::clone(&context.agent_manager),
                            context.working_dir.clone(),
                            context.event_tx.clone(),
                        );
                    }
                    KeyHandlerResult::StartConversation(input) => {
                        agent_task = Some(start_agent_conversation(
                            input,
                            Arc::clone(&context.parser),
                            Arc::clone(&context.conversation),
                            Arc::clone(&context.backend),
                            Arc::clone(&context.tool_registry),
                            Arc::clone(&context.tool_executor),
                            context.event_tx.clone(),
                        ));
                    }
                    _ => {}
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

    Ok(())
}
