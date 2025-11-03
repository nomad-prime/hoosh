use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub trait Component: Send + Sync {
    type State;
    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer);
}
