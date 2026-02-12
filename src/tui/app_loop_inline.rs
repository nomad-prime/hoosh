use anyhow::Result;
use crossterm::event;
use std::time::Duration;
use tokio::task::JoinHandle;

use super::app_state::AppState;
use super::message_renderer::MessageRenderer;
use crate::agent::AgentEvent;
use crate::console::{VerbosityLevel, console};
use crate::tui::actions::{answer, execute_command};
use crate::tui::app_layout::AppLayout;
use crate::tui::layout::Layout;
use crate::tui::terminal::lifecycle_inline::{HooshTerminal, resize_terminal_inline};

pub use super::app_loop::EventLoopContext;

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
    resize_terminal_inline(terminal, layout.total_height())?;

    terminal.draw(|frame| {
        let area = frame.area();
        layout.render(app, area, frame.buffer_mut());

        // Position cursor in the input field
        let input_component_area = calculate_input_area(&layout, area);

        // Match the input component's area calculation exactly
        use ratatui::widgets::{Block, Borders};
        use ratatui::layout::{Constraint, Layout as RatatuiLayout};

        let input_block = Block::default().borders(Borders::BOTTOM | Borders::TOP);
        let inner_area = input_block.inner(input_component_area);

        // Calculate text_area exactly like the input component does
        let horizontal = RatatuiLayout::horizontal([
            Constraint::Length(2), // Prompt area ("> ")
            Constraint::Min(1),    // Text input area
        ]);
        let areas = horizontal.split(inner_area);
        let text_area = areas[1]; // The second area is the text area

        // Get cursor position from textarea
        // Note: There seems to be an off-by-one issue, possibly in how the textarea
        // calculates column width. Subtracting 1 appears to fix it.
        if let Some((cursor_x, cursor_y)) = app.input.cursor_pos(text_area) {
            frame.set_cursor_position(ratatui::layout::Position {
                x: cursor_x,
                y: cursor_y,
            });
        }
    })?;

    Ok(())
}

fn calculate_input_area(layout: &Layout<AppState>, area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    // Replicate the layout calculation to find where the input component is
    use ratatui::layout::{Constraint, Layout as RatatuiLayout};

    let constraints: Vec<Constraint> = layout
        .visible_components()
        .map(|comp| Constraint::Length(comp.height()))
        .collect();

    let areas = RatatuiLayout::vertical(constraints).split(area);

    // Find the input component (it's typically second to last, before mode indicator)
    let input_index = layout
        .visible_components()
        .count()
        .saturating_sub(2); // -2 because mode indicator is last, input is second to last

    areas.get(input_index).copied().unwrap_or(area)
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
