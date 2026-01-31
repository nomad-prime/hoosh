use anyhow::Result;
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::traits::TerminalMode;

#[derive(Default)]
pub struct TaggedMode {}

impl TaggedMode {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TerminalMode for TaggedMode {
    fn render(&self, _area: Rect, _buf: &mut Buffer) -> Result<()> {
        Ok(())
    }

    fn handle_event(&mut self, _event: Event) -> Result<bool> {
        Ok(false)
    }

    fn mode_name(&self) -> &'static str {
        "tagged"
    }
}
