// Trait definitions for terminal mode implementations

use anyhow::Result;
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Terminal mode trait for different rendering and input handling strategies
pub trait TerminalMode: Send + Sync {
    /// Render the current state to the buffer
    fn render(&self, area: Rect, buf: &mut Buffer) -> Result<()>;

    /// Handle input events
    fn handle_event(&mut self, event: Event) -> Result<bool>;

    /// Get the mode name for logging/debugging
    fn mode_name(&self) -> &'static str;
}
