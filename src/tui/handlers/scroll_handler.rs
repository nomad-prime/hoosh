use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use crate::tui::state::AppState;
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
                    let page = app.scroll.page();
                    app.scroll.down(page);
                    KeyHandlerResult::Handled
                }
                KeyCode::PageUp => {
                    let page = app.scroll.page();
                    app.scroll.up(page);
                    KeyHandlerResult::Handled
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let half = app.scroll.half_page();
                    app.scroll.down(half);
                    KeyHandlerResult::Handled
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let half = app.scroll.half_page();
                    app.scroll.up(half);
                    KeyHandlerResult::Handled
                }
                _ => KeyHandlerResult::NotHandled,
            },
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                ..
            }) => {
                app.scroll.up(3);
                KeyHandlerResult::Handled
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                ..
            }) => {
                app.scroll.down(3);
                KeyHandlerResult::Handled
            }
            _ => KeyHandlerResult::NotHandled,
        }
    }
}
