use anyhow::Result;
use crossterm::event;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use super::actions::{execute_command, start_agent_conversation};
use super::app::AppState;
use super::input_handler::InputHandler;
use super::message_renderer::MessageRenderer;
use crate::agents::AgentManager;
use crate::backends::LlmBackend;
use crate::commands::CommandRegistry;
use crate::config::AppConfig;
use crate::conversations::{AgentEvent, Conversation, MessageSummarizer};
use crate::parser::MessageParser;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;
use crate::tui::app_layout::AppLayout;
use crate::tui::layout_builder::Layout;
use crate::tui::terminal::{HooshTerminal, resize_terminal};
use crate::tui::ui::render_ui;

pub struct SystemResources {
    pub backend: Arc<dyn LlmBackend>,
    pub parser: Arc<MessageParser>,
    pub tool_registry: Arc<ToolRegistry>,
    pub tool_executor: Arc<ToolExecutor>,
    pub agent_manager: Arc<AgentManager>,
    pub command_registry: Arc<CommandRegistry>,
}

pub struct ConversationState {
    pub conversation: Arc<Mutex<Conversation>>,
    pub summarizer: Arc<MessageSummarizer>,
    pub current_agent_name: String,
}

pub struct EventChannels {
    pub event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    pub event_tx: mpsc::UnboundedSender<AgentEvent>,
}

pub struct RuntimeState {
    pub permission_manager: Arc<crate::permissions::PermissionManager>,
    pub input_handlers: Vec<Box<dyn InputHandler + Send>>,
    pub working_dir: String,
    pub config: AppConfig,
}

pub struct EventLoopContext {
    pub system: SystemResources,
    pub conversation: ConversationState,
    pub channels: EventChannels,
    pub runtime: RuntimeState,
}

pub async fn run_event_loop(
    mut terminal: HooshTerminal,
    app: &mut AppState,
    mut context: EventLoopContext,
) -> Result<HooshTerminal> {
    let mut agent_task: Option<JoinHandle<()>> = None;

    let message_renderer = MessageRenderer::new();

    loop {
        message_renderer.render_pending_messages(app, &mut terminal)?;

        let layout = Layout::create(app);

        resize_terminal(&mut terminal, layout.total_height())?;

        terminal.draw(|frame| {
            render_ui(frame, app, &layout);
        })?;

        while let Ok(event) = context.channels.event_rx.try_recv() {
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
                    let mut conv = context.conversation.conversation.lock().await;
                    conv.messages.clear();
                    app.add_message("Conversation cleared.\n".to_string());
                }
                AgentEvent::DebugMessage(_msg) => {
                    // Debug messages are currently suppressed
                    // app.add_message(format!("[DEBUG] {}\n", _msg));
                }
                AgentEvent::AgentSwitched { new_agent_name } => {
                    context.conversation.current_agent_name = new_agent_name;
                    // The header will be updated on next render
                }
                other_event => {
                    app.handle_agent_event(other_event);
                }
            }
        }

        if let Some(task) = &agent_task
            && task.is_finished()
        {
            agent_task = None;
        }

        app.tick_animation();

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            let agent_task_active = agent_task.is_some();

            // Iterate through handlers in order until one handles the event
            for handler in &mut context.runtime.input_handlers {
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
                                "Task cancelled by user (press Ctrl+C again to quit)\n",
                            );
                        }
                        app.should_cancel_task = false;
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::StartCommand(input)) => {
                        use super::actions::CommandExecutionContext;
                        execute_command(CommandExecutionContext {
                            input,
                            command_registry: Arc::clone(&context.system.command_registry),
                            conversation: Arc::clone(&context.conversation.conversation),
                            tool_registry: Arc::clone(&context.system.tool_registry),
                            agent_manager: Arc::clone(&context.system.agent_manager),
                            working_dir: context.runtime.working_dir.clone(),
                            event_tx: context.channels.event_tx.clone(),
                            permission_manager: Arc::clone(&context.runtime.permission_manager),
                            summarizer: Arc::clone(&context.conversation.summarizer),
                            current_agent_name: context.conversation.current_agent_name.clone(),
                            config: context.runtime.config.clone(),
                            backend: Arc::clone(&context.system.backend),
                        });
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::StartConversation(input)) => {
                        agent_task = Some(start_agent_conversation(
                            input,
                            Arc::clone(&context.system.parser),
                            Arc::clone(&context.conversation.conversation),
                            Arc::clone(&context.system.backend),
                            Arc::clone(&context.system.tool_registry),
                            Arc::clone(&context.system.tool_executor),
                            context.channels.event_tx.clone(),
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
