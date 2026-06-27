use anyhow::Result;
use crossterm::event;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use super::app_state::AppState;
use super::input_handler::InputHandler;
use super::message_renderer::MessageRenderer;
use crate::agent::{AgentEvent, CancelKind, Conversation};
use crate::agent_definition::AgentDefinitionManager;
use crate::backends::LlmBackend;
use crate::commands::CommandRegistry;
use crate::config::AppConfig;
use crate::console::{VerbosityLevel, console};
use crate::context_management::ContextManager;
use crate::memory_mode::MemoryModeManager;
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
    pub memory_mode_manager: Option<Arc<MemoryModeManager>>,
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

        cleanup_finished_task(&mut agent_task, app);
        start_next_queued_prompt(&mut agent_task, app, &context);

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
    let terminal_height = terminal.size()?.height;
    let layout = Layout::create(app, terminal_width, terminal_height);
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
            tracing::debug!(target: "hoosh::agent", "{}", msg);
            if console().verbosity() >= VerbosityLevel::Debug {
                app.add_debug_message(msg);
            }
        }
        AgentEvent::SwitchBackend {
            backend,
            model,
            save,
        } => {
            apply_backend_switch(app, context, backend, model, save);
        }
        other_event => {
            app.handle_agent_event(other_event);
        }
    }
}

pub(crate) fn apply_backend_switch(
    app: &mut AppState,
    context: &mut EventLoopContext,
    new_backend: Option<String>,
    new_model: Option<String>,
    save: bool,
) {
    let target_backend = new_backend
        .clone()
        .unwrap_or_else(|| context.runtime.config.default_backend.clone());

    // Stage the change on a clone so a failure leaves the live config untouched.
    let mut staged = context.runtime.config.clone();
    if let Some(ref b) = new_backend {
        staged.default_backend = b.clone();
    }
    if let Some(ref m) = new_model {
        if let Some(cfg) = staged.backends.get_mut(&target_backend) {
            cfg.model = Some(m.clone());
        } else {
            app.add_status_message(&format!(
                "Switch failed: backend '{target_backend}' not configured\n"
            ));
            return;
        }
    }

    let built = crate::backends::backend_factory::create_backend(&target_backend, &staged);
    let new_backend_arc: Arc<dyn LlmBackend> = match built {
        Ok(b) => Arc::from(b),
        Err(e) => {
            app.add_status_message(&format!("Switch failed: {e}\n"));
            return;
        }
    };

    context.runtime.config = staged;
    context.system_resources.backend = new_backend_arc;

    let mut summary = format!(
        "Switched to backend '{}' (model: {})",
        context.system_resources.backend.backend_name(),
        context.system_resources.backend.model_name()
    );
    if let Some(p) = context.system_resources.backend.pricing() {
        summary.push_str(&format!(
            " — ${:.2}/M in, ${:.2}/M out",
            p.input_per_million, p.output_per_million
        ));
    }
    if save {
        match context.runtime.config.save() {
            Ok(()) => summary.push_str(" [saved]"),
            Err(e) => summary.push_str(&format!(" [save failed: {e}]")),
        }
    }
    summary.push('\n');
    app.add_status_message(&summary);
    tracing::info!(
        target: "hoosh::session",
        "backend switched to {} / {}",
        context.system_resources.backend.backend_name(),
        context.system_resources.backend.model_name()
    );
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
        if process_handler_result(result, app, agent_task, context).await {
            break;
        }
    }

    Ok(())
}

async fn process_handler_result(
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
            handle_cancel_task(app, agent_task, context).await;
            true
        }
        KeyHandlerResult::StartCommand(input) => {
            execute_command(input, context);
            true
        }
        KeyHandlerResult::StartConversation {
            input,
            image_attachments,
        } => {
            *agent_task = Some(answer(input, image_attachments, context));
            true
        }
    }
}

/// Shared cancel implementation for all three event-loop variants.
///
/// Splits behaviour by whether any tool calls fired this turn:
///   - **Tool-cancel**: render an `[cancelled by user]` marker under each
///     in-flight tool card, inject synthetic tool results into the
///     conversation so the next turn the model knows, and keep the
///     submitted prompt out of the input (turn is consumed in history).
///   - **Thinking-cancel**: drop the turn from history, restore the prompt
///     to the input, show a single "Task cancelled by user" status line.
pub(crate) async fn handle_cancel_task(
    app: &mut AppState,
    agent_task: &mut Option<JoinHandle<()>>,
    context: &EventLoopContext,
) {
    if let Some(task) = agent_task.take() {
        task.abort();
        app.agent_state = super::events::AgentState::Idle;
        app.hide_approval_dialog();
        app.hide_tool_permission_dialog();

        // Keep whatever text had already streamed in so the user doesn't lose
        // what they were reading when they interrupted.
        if app.streaming.to_scrollback {
            if app.streaming.is_active() {
                app.streaming.finalize = true;
            }
        } else if let Some(partial) = app.streaming.text.take()
            && !partial.trim().is_empty()
        {
            app.add_final_response(partial.trim_end());
        }

        let kind = {
            let mut conv = context.conversation_state.conversation.lock().await;
            conv.cancel_in_flight_turn()
        };

        match kind {
            CancelKind::Tool { .. } => {
                use ratatui::style::Style;
                use ratatui::text::{Line, Span};
                let cancelled = std::mem::take(&mut app.active_tool_calls);
                for tc in &cancelled {
                    app.add_message("".to_string());
                    app.add_styled_line(Line::from(vec![
                        Span::styled(
                            super::glyphs::TOOL_ERROR,
                            Style::default().fg(super::colors::palette::DESTRUCTIVE),
                        ),
                        Span::raw(format!(" {}", tc.display_name)),
                    ]));
                    app.add_status_message("cancelled by user");
                }
                app.last_submitted_input = None;
            }
            CancelKind::Thinking => {
                use ratatui::style::{Modifier, Style};
                use ratatui::text::{Line, Span};
                app.clear_active_tool_calls();
                app.add_styled_line(Line::from(Span::styled(
                    super::app_state::format_inline_status("retracted, hoosh did not see this"),
                    Style::default()
                        .fg(super::colors::palette::DIMMED_TEXT)
                        .add_modifier(Modifier::ITALIC),
                )));
                app.add_message("".to_string());
                restore_cancelled_prompt(app);
            }
        }

        let dropped = app.queued_prompts.len();
        if dropped > 0 {
            app.queued_prompts.clear();
            app.add_status_message(&format!(
                "Dropped {dropped} queued prompt{}\n",
                if dropped == 1 { "" } else { "s" }
            ));
        }
    }
    app.should_cancel_task = false;
}

/// Put the prompt that started the cancelled turn back into the input buffer,
/// but only if the user hasn't already started typing something new.
pub(crate) fn restore_cancelled_prompt(app: &mut AppState) {
    if let Some(prompt) = app.last_submitted_input.take()
        && app.get_input_text().is_empty()
    {
        app.set_input_text(&prompt);
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
    app.metrics.reset();
    app.add_status_message("Conversation cleared.");
}

fn cleanup_finished_task(agent_task: &mut Option<JoinHandle<()>>, app: &mut AppState) {
    if let Some(task) = agent_task
        && task.is_finished()
    {
        *agent_task = None;
        // Turn ended naturally — drop the snapshot so a later idle Ctrl+C
        // doesn't restore a prompt that already ran.
        app.last_submitted_input = None;
    }
}

/// If the agent task just finished (or never started) and the user queued
/// prompts mid-flight, dequeue the next one and start it as a new turn.
/// Slash commands queued this way fire as commands via `execute_command`.
pub(crate) fn start_next_queued_prompt(
    agent_task: &mut Option<JoinHandle<()>>,
    app: &mut AppState,
    context: &EventLoopContext,
) {
    if agent_task.is_some() {
        return;
    }
    let Some(next) = app.queued_prompts.pop_front() else {
        return;
    };
    app.add_user_input(&next);
    if next.trim().starts_with('/') {
        app.last_submitted_input = None;
        execute_command(next, context);
    } else {
        app.last_submitted_input = Some(next.clone());
        // Queued prompts never carry image attachments — those flow through
        // the inline submit path. v1: keep the queue text-only.
        *agent_task = Some(answer(next, Vec::new(), context));
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;

    #[test]
    fn restore_puts_prompt_back_when_input_is_empty() {
        let mut app = AppState::new();
        app.last_submitted_input = Some("hello world".to_string());

        restore_cancelled_prompt(&mut app);

        assert_eq!(app.get_input_text(), "hello world");
        assert!(app.last_submitted_input.is_none());
    }

    #[test]
    fn restore_leaves_input_alone_when_user_already_typed() {
        let mut app = AppState::new();
        app.last_submitted_input = Some("original".to_string());
        app.set_input_text("user is typing something new");

        restore_cancelled_prompt(&mut app);

        assert_eq!(app.get_input_text(), "user is typing something new");
        // Snapshot is still consumed — we don't want it to leak into a later cancel.
        assert!(app.last_submitted_input.is_none());
    }

    #[test]
    fn restore_with_no_snapshot_is_noop() {
        let mut app = AppState::new();
        restore_cancelled_prompt(&mut app);
        assert_eq!(app.get_input_text(), "");
    }
}
