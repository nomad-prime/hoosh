use super::app::AppState;
use crate::tui::layout_builder::Layout;
use crate::tui::terminal::Frame;

/// Renders the entire UI into a frame.
/// It assumes the frame it receives is *already* the correct size.
pub fn render_ui(frame: &mut Frame, app: &AppState, layout: &Layout<AppState>) {
    let viewport_area = frame.area();
    layout.render(app, viewport_area, frame.buffer_mut());
}
