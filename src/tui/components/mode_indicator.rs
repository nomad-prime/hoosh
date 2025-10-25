use crate::tui::app::AppState;
use crate::tui::layout_builder::WidgetRenderer;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct ModeIndicatorRenderer;

impl WidgetRenderer for ModeIndicatorRenderer {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let is_autopilot = state
            .autopilot_enabled
            .load(std::sync::atomic::Ordering::Relaxed);

        let mode_text = if is_autopilot {
            " ⏵⏵ Autopilot"
        } else {
            "  ⏸ Review"
        };

        let mode_color = if is_autopilot {
            Color::Rgb(142, 240, 204)
        } else {
            Color::Magenta
        };

        let mode_line = Line::from(vec![
            Span::styled(
                mode_text,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " (shift+tab to toggle)",
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
            ),
        ]);

        let paragraph = Paragraph::new(mode_line);
        paragraph.render(area, buf);
    }
}
