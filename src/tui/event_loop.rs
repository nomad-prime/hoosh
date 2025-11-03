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
use crate::console::{VerbosityLevel, console};
use crate::context_management::{ContextManager, MessageSummarizer};
use crate::conversations::{AgentEvent, Conversation};
use crate::parser::MessageParser;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;
use crate::tui::app_layout::AppLayout;
use crate::tui::layout::Layout;
use crate::tui::terminal::{HooshTerminal, resize_terminal};

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
    pub context_manager: Arc<ContextManager>,
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
    pub system_resources: SystemResources,
    pub conversation_state: ConversationState,
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
            layout.render(app, frame.area(), frame.buffer_mut());
        })?;

        while let Ok(event) = context.channels.event_rx.try_recv() {
            match event {
                AgentEvent::ToolPermissionRequest {
                    descriptor,
                    request_id,
                } => {
                    app.show_tool_permission_dialog(descriptor, request_id);
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
                    let mut conv = context.conversation_state.conversation.lock().await;
                    conv.messages.clear();
                    context
                        .conversation_state
                        .context_manager
                        .token_accountant
                        .reset();
                    app.input_tokens = 0;
                    app.output_tokens = 0;
                    app.total_cost = 0.0;
                    app.add_message("Conversation cleared.\n".to_string());
                }
                AgentEvent::DebugMessage(msg) => {
                    if console().verbosity() >= VerbosityLevel::Debug {
                        app.add_debug_message(msg);
                    }
                }
                AgentEvent::AgentSwitched { new_agent_name } => {
                    context.conversation_state.current_agent_name = new_agent_name;
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
                        execute_command(input, &context);
                        break;
                    }
                    Ok(super::handler_result::KeyHandlerResult::StartConversation(input)) => {
                        agent_task = Some(start_agent_conversation(input, &context));
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
