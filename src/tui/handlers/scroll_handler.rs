use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseEvent, MouseEventKind};

pub struct ScrollHandler;

impl Default for ScrollHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl InputHandler for ScrollHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::PageDown => {
                    let max_scroll = app.vertical_scroll_content_length
                        .saturating_sub(app.vertical_scroll_viewport_length);
                    app.vertical_scroll = app
                        .vertical_scroll
                        .saturating_add(app.vertical_scroll_viewport_length.saturating_sub(1))
                        .min(max_scroll);
                    app.vertical_scroll_state =
                        app.vertical_scroll_state.position(app.vertical_scroll);
                    KeyHandlerResult::Handled
                }
                KeyCode::PageUp => {
                    app.vertical_scroll = app
                        .vertical_scroll
                        .saturating_sub(app.vertical_scroll_viewport_length.saturating_sub(1));
                    app.vertical_scroll_state =
                        app.vertical_scroll_state.position(app.vertical_scroll);
                    KeyHandlerResult::Handled
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let half_page = app.vertical_scroll_viewport_length / 2;
                    let max_scroll = app.vertical_scroll_content_length
                        .saturating_sub(app.vertical_scroll_viewport_length);
                    app.vertical_scroll = app.vertical_scroll.saturating_add(half_page).min(max_scroll);
                    app.vertical_scroll_state =
                        app.vertical_scroll_state.position(app.vertical_scroll);
                    KeyHandlerResult::Handled
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let half_page = app.vertical_scroll_viewport_length / 2;
                    app.vertical_scroll = app.vertical_scroll.saturating_sub(half_page);
                    app.vertical_scroll_state =
                        app.vertical_scroll_state.position(app.vertical_scroll);
                    KeyHandlerResult::Handled
                }
                _ => KeyHandlerResult::NotHandled,
            },
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                ..
            }) => {
                app.vertical_scroll = app.vertical_scroll.saturating_sub(3);
                let max_scroll = app.vertical_scroll_content_length
                    .saturating_sub(app.vertical_scroll_viewport_length);
                app.vertical_scroll = app.vertical_scroll.min(max_scroll);
                app.vertical_scroll_state = app.vertical_scroll_state.position(app.vertical_scroll);
                KeyHandlerResult::Handled
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                ..
            }) => {
                let max_scroll = app.vertical_scroll_content_length
                    .saturating_sub(app.vertical_scroll_viewport_length);
                app.vertical_scroll = app.vertical_scroll.saturating_add(3).min(max_scroll);
                app.vertical_scroll_state = app.vertical_scroll_state.position(app.vertical_scroll);
                KeyHandlerResult::Handled
            }
            _ => KeyHandlerResult::NotHandled,
        }
    }
}
