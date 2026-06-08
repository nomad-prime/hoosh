use crate::tui::app_state::AppState;
use crate::tui::colors::palette;
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

const MAX_VISIBLE: usize = 5;

/// Shows prompts the user typed while the agent was busy, stacked just above
/// the input bar so it's obvious what will run next. Coexists with the todo
/// list, which renders in its own region directly above this one.
pub struct QueuedPromptsComponent;

impl Component for QueuedPromptsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        if state.queued_prompts.is_empty() {
            return;
        }

        let total = state.queued_prompts.len();
        let visible = total.min(MAX_VISIBLE);
        let mut lines: Vec<Line> = Vec::with_capacity(visible + 1);

        // Header: total count.
        lines.push(Line::from(vec![Span::styled(
            format!("⏳ {total} queued"),
            Style::default().fg(palette::SECONDARY_TEXT),
        )]));

        for (idx, prompt) in state.queued_prompts.iter().take(MAX_VISIBLE).enumerate() {
            let trimmed: String = prompt.chars().take(80).collect();
            let suffix = if prompt.chars().count() > 80 {
                "…"
            } else {
                ""
            };
            let prefix = if idx == 0 { "⎿ " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(palette::SUBDUED_TEXT)),
                Span::styled(
                    format!("{idx}. ", idx = idx + 1),
                    Style::default().fg(palette::SUBDUED_TEXT),
                ),
                Span::styled(trimmed, Style::default().fg(palette::PRIMARY_TEXT)),
                Span::styled(
                    suffix.to_string(),
                    Style::default().fg(palette::SUBDUED_TEXT),
                ),
            ]));
        }

        if total > MAX_VISIBLE {
            lines.push(Line::from(vec![Span::styled(
                format!("  …and {} more", total - MAX_VISIBLE),
                Style::default().fg(palette::SUBDUED_TEXT),
            )]));
        }

        Paragraph::new(lines).render(area, buf);
    }
}
