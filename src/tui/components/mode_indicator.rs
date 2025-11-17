use crate::tui::app_state::AppState;
use crate::tui::component::Component;
use crate::tui::palette;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct ModeIndicator;

impl Component for ModeIndicator {
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
            palette::STATUS_IDLE
        } else {
            palette::MARKDOWN_HEADING
        };

        let mode_line = Line::from(vec![
            Span::styled(
                mode_text,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " (shift+tab to toggle)",
                Style::default()
                    .fg(palette::SECONDARY_TEXT)
                    .add_modifier(Modifier::DIM),
            ),
        ]);

        let paragraph = Paragraph::new(mode_line);
        paragraph.render(area, buf);
    }
}
