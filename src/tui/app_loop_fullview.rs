use anyhow::Result;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::interval;

use super::app_state::AppState;
use super::app_state::MessageLine;
use super::message_renderer::MessageRenderer;
use crate::agent::AgentEvent;
use crate::console::{VerbosityLevel, console};
use crate::tui::actions::{answer, execute_command};
use crate::tui::app_layout::AppLayout;
use crate::tui::layout::Layout;
use crate::tui::terminal::lifecycle_fullview::HooshTerminal;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, StatefulWidget, Widget, Wrap};

pub use super::app_loop::EventLoopContext;

pub async fn run_event_loop(
    mut terminal: HooshTerminal,
    app: &mut AppState,
    mut context: EventLoopContext,
) -> Result<HooshTerminal> {
    let mut agent_task: Option<JoinHandle<()>> = None;

    let message_renderer = MessageRenderer::new();
    let mut event_stream = EventStream::new();
    let mut render_interval = interval(Duration::from_millis(50));
    let mut tick_interval = interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            _ = render_interval.tick() => {
                render_frame(app, &mut terminal, &message_renderer)?;
            }
            _ = tick_interval.tick() => {
                process_agent_events(app, &mut context).await;
                cleanup_finished_task(&mut agent_task);

                let should_animate = matches!(app.agent_state, super::events::AgentState::Thinking | super::events::AgentState::ExecutingTools);
                if should_animate {
                    app.tick_animation();
                }
            }
            Some(Ok(event)) = event_stream.next() => {
                let is_mouse_scroll = matches!(
                    event,
                    Event::Mouse(ref m) if matches!(m.kind, crossterm::event::MouseEventKind::ScrollUp | crossterm::event::MouseEventKind::ScrollDown)
                );

                handle_user_input(&event, app, &mut agent_task, &mut context).await?;

                if !is_mouse_scroll {
                    render_frame(app, &mut terminal, &message_renderer)?;
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

    Ok(terminal)
}

fn render_frame(
    app: &mut AppState,
    terminal: &mut HooshTerminal,
    _message_renderer: &MessageRenderer,
) -> Result<()> {
    let has_pending = app.has_pending_messages();
    let _ = app.drain_pending_messages();

    terminal.draw(|frame| {
        let area = frame.area();
        let layout = Layout::create(app);
        let ui_height = layout.total_height();

        let message_area = ratatui::layout::Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(ui_height),
        };

        let ui_area = ratatui::layout::Rect {
            x: area.x,
            y: area.height.saturating_sub(ui_height),
            width: area.width,
            height: ui_height,
        };

        let viewport_height = message_area.height as usize;
        app.vertical_scroll_viewport_length = viewport_height;

        let content_width = if app.vertical_scroll_content_length > viewport_height {
            message_area.width.saturating_sub(1)
        } else {
            message_area.width
        } as usize;

        let was_at_bottom = app.vertical_scroll
            >= app
                .vertical_scroll_content_length
                .saturating_sub(app.vertical_scroll_viewport_length);

        app.vertical_scroll_content_length = calculate_wrapped_line_count(app, content_width)
            .saturating_add(10);

        if has_pending && was_at_bottom {
            app.vertical_scroll = app
                .vertical_scroll_content_length
                .saturating_sub(viewport_height);
        }

        let max_scroll = app
            .vertical_scroll_content_length
            .saturating_sub(viewport_height);
        app.vertical_scroll = app.vertical_scroll.min(max_scroll);

        app.vertical_scroll_state = app
            .vertical_scroll_state
            .content_length(app.vertical_scroll_content_length)
            .viewport_content_length(viewport_height)
            .position(app.vertical_scroll);

        render_messages_fullview(app, message_area, frame.buffer_mut());
        layout.render(app, ui_area, frame.buffer_mut());
    })?;

    Ok(())
}

fn calculate_wrapped_line_count(app: &AppState, content_width: usize) -> usize {
    use crate::tui::markdown::MarkdownRenderer;

    let markdown_renderer = MarkdownRenderer::new();
    let mut total_lines = 0;

    for ml in app.messages.iter() {
        total_lines += match ml {
            MessageLine::Plain(text) => {
                use crate::tui::text_utils;
                let clean_text = text_utils::strip_ansi_codes(text);
                calculate_wrapped_lines_for_text(&clean_text, content_width)
            }
            MessageLine::Styled(line) => {
                let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                if line_text.is_empty() {
                    1
                } else {
                    textwrap::wrap(&line_text, content_width).len()
                }
            }
            MessageLine::Markdown(md) => {
                let rendered = markdown_renderer.render(md);
                calculate_wrapped_lines_for_styled_lines(&rendered, content_width)
            }
        };
    }

    total_lines
}

fn calculate_wrapped_lines_for_text(text: &str, width: usize) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut line_count = 0;
    for line in text.lines() {
        if line.is_empty() {
            line_count += 1;
        } else {
            line_count += textwrap::wrap(line, width).len();
        }
    }

    line_count.max(1)
}

fn calculate_wrapped_lines_for_styled_lines(lines: &[Line], width: usize) -> usize {
    let mut total = 0;
    for line in lines {
        let text_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if text_width <= width {
            total += 1;
        } else {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            total += textwrap::wrap(&line_text, width).len().max(1);
        }
    }
    total
}

fn render_messages_fullview(
    app: &mut AppState,
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
) {
    use crate::tui::markdown::MarkdownRenderer;

    let markdown_renderer = MarkdownRenderer::new();
    let mut all_lines: Vec<Line> = Vec::new();

    for ml in app.messages.iter() {
        match ml {
            MessageLine::Plain(text) => {
                use crate::tui::text_utils;
                let clean_text = text_utils::strip_ansi_codes(text);
                for line_text in clean_text.lines() {
                    all_lines.push(Line::from(Span::raw(line_text.to_string())));
                }
            }
            MessageLine::Styled(line) => {
                all_lines.push(line.clone());
            }
            MessageLine::Markdown(md) => {
                let rendered = markdown_renderer.render(md);
                all_lines.extend(rendered);
            }
        }
    }

    let viewport_height = area.height as usize;

    let content_area = if app.vertical_scroll_content_length > viewport_height {
        ratatui::layout::Rect {
            x: area.x,
            y: area.y,
            width: area.width.saturating_sub(1),
            height: area.height,
        }
    } else {
        area
    };

    Paragraph::new(all_lines)
        .wrap(Wrap { trim: false })
        .scroll((app.vertical_scroll as u16, 0))
        .render(content_area, buf);

    if app.vertical_scroll_content_length > viewport_height {
        let scrollbar_area = ratatui::layout::Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        scrollbar.render(scrollbar_area, buf, &mut app.vertical_scroll_state);
    }
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
    event: &Event,
    app: &mut AppState,
    agent_task: &mut Option<JoinHandle<()>>,
    context: &mut EventLoopContext,
) -> Result<()> {
    let agent_task_active = agent_task.is_some();

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
    app.add_message("Conversation cleared.\n".to_string());
}

fn cleanup_finished_task(agent_task: &mut Option<JoinHandle<()>>) {
    if let Some(task) = agent_task
        && task.is_finished()
    {
        *agent_task = None;
    }
}
