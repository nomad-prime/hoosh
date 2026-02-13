use anyhow::Result;
use crossterm::event;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use super::app_state::AppState;
use super::input_handler::InputHandler;
use super::message_renderer::MessageRenderer;
use crate::agent::{AgentEvent, Conversation};
use crate::agent_definition::AgentDefinitionManager;
use crate::backends::LlmBackend;
use crate::commands::CommandRegistry;
use crate::config::AppConfig;
use crate::console::{VerbosityLevel, console};
use crate::context_management::{ContextManager, MessageSummarizer};
use crate::parser::MessageParser;
use crate::storage::ConversationStorage;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;
use crate::tools::todo_state::TodoState;
use crate::tui::actions::{answer, execute_command};
use crate::tui::app_layout::AppLayout;
use crate::tui::layout::Layout;
use crate::tui::terminal::{HooshTerminal, resize_terminal};

pub struct SystemResources {
    pub backend: Arc<dyn LlmBackend>,
    pub parser: Arc<MessageParser>,
    pub tool_registry: Arc<ToolRegistry>,
    pub tool_executor: Arc<ToolExecutor>,
    pub agent_manager: Arc<AgentDefinitionManager>,
    pub command_registry: Arc<CommandRegistry>,
    pub system_reminder: Arc<crate::system_reminders::SystemReminder>,
}

pub struct ConversationState {
    pub conversation: Arc<Mutex<Conversation>>,
    pub summarizer: Arc<MessageSummarizer>,
    pub context_manager: Arc<ContextManager>,
    pub current_agent_name: String,
    pub conversation_storage: Arc<ConversationStorage>,
    pub conversation_id: String,
}

pub struct EventChannels {
    pub event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    pub event_tx: mpsc::UnboundedSender<AgentEvent>,
}

pub struct TaggedModeChannels {
    pub permission_response_tx: mpsc::UnboundedSender<crate::agent::PermissionResponse>,
    pub approval_response_tx: mpsc::UnboundedSender<crate::agent::ApprovalResponse>,
}

pub struct RuntimeState {
    pub permission_manager: Arc<crate::permissions::PermissionManager>,
    pub input_handlers: Vec<Box<dyn InputHandler + Send>>,
    pub working_dir: String,
    pub config: AppConfig,
    pub todo_state: TodoState,
}

pub struct EventLoopContext {
    pub system_resources: SystemResources,
    pub conversation_state: ConversationState,
    pub channels: EventChannels,
    pub runtime: RuntimeState,
    pub tagged_mode_channels: TaggedModeChannels,
}

pub async fn run_event_loop(
    mut terminal: HooshTerminal,
    app: &mut AppState,
    mut context: EventLoopContext,
) -> Result<HooshTerminal> {
    let mut agent_task: Option<JoinHandle<()>> = None;

    let message_renderer = MessageRenderer::new();

    loop {
        render_frame(app, &mut terminal, &message_renderer)?;

        process_agent_events(app, &mut context).await;

        cleanup_finished_task(&mut agent_task);

        app.tick_animation();

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            handle_user_input(&event, app, &mut agent_task, &mut context).await?;
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

fn render_frame(
    app: &mut AppState,
    terminal: &mut HooshTerminal,
    message_renderer: &MessageRenderer,
) -> Result<()> {
    message_renderer.render_pending_messages(app, terminal)?;

    let terminal_width = terminal.get_viewport_area().width;
    let layout = Layout::create(app, terminal_width);
    resize_terminal(terminal, layout.total_height())?;

    terminal.draw(|frame| {
        layout.render(app, frame.area(), frame.buffer_mut());
    })?;

    Ok(())
}

async fn process_agent_events(app: &mut AppState, context: &mut EventLoopContext) {
    while let Ok(event) = context.channels.event_rx.try_recv() {
        handle_agent_event(app, event, context).await;
    }
}

async fn handle_agent_event(app: &mut AppState, event: AgentEvent, context: &mut EventLoopContext) {
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
            clear_conversation(app, context).await;
        }
        AgentEvent::DebugMessage(msg) => {
            if console().verbosity() >= VerbosityLevel::Debug {
                app.add_debug_message(msg);
            }
        }
        other_event => {
            app.handle_agent_event(other_event);
        }
    }
}

async fn handle_user_input(
    event: &event::Event,
    app: &mut AppState,
    agent_task: &mut Option<JoinHandle<()>>,
    context: &mut EventLoopContext,
) -> Result<()> {
    let agent_task_active = agent_task.is_some();

    // Process handlers one at a time, stopping when one handles the event
    let handler_count = context.runtime.input_handlers.len();
    for i in 0..handler_count {
        let result = context.runtime.input_handlers[i]
            .handle_event(event, app, agent_task_active)
            .await;
        if process_handler_result(result, app, agent_task, context) {
            break;
        }
    }

    Ok(())
}

fn process_handler_result(
    result: super::handler_result::KeyHandlerResult,
    app: &mut AppState,
    agent_task: &mut Option<JoinHandle<()>>,
    context: &EventLoopContext,
) -> bool {
    use super::handler_result::KeyHandlerResult;

    match result {
        KeyHandlerResult::NotHandled => false,
        KeyHandlerResult::Handled => true,
        KeyHandlerResult::ShouldQuit => {
            app.should_quit = true;
            if let Some(task) = agent_task.take() {
                task.abort();
            }
            true
        }
        KeyHandlerResult::ShouldCancelTask => {
            if let Some(task) = agent_task.take() {
                task.abort();
                app.agent_state = super::events::AgentState::Idle;
                app.hide_approval_dialog();
                app.hide_tool_permission_dialog();
                app.clear_active_tool_calls();
                app.add_status_message("Task cancelled by user (press Ctrl+C again to quit)\n");
            }
            app.should_cancel_task = false;
            true
        }
        KeyHandlerResult::StartCommand(input) => {
            execute_command(input, context);
            true
        }
        KeyHandlerResult::StartConversation(input) => {
            *agent_task = Some(answer(input, context));
            true
        }
    }
}

async fn clear_conversation(app: &mut AppState, context: &mut EventLoopContext) {
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
    app.add_status_message("Conversation cleared.");
}

fn cleanup_finished_task(agent_task: &mut Option<JoinHandle<()>>) {
    if let Some(task) = agent_task
        && task.is_finished()
    {
        *agent_task = None;
    }
}
