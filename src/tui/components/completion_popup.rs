use crate::tui::app_state::AppState;
use crate::tui::component::Component;
use crate::tui::palette;
use ratatui::text::Span;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Widget},
};

pub struct CompletionPopup;

impl Component for CompletionPopup {
    type State = AppState;
    fn render(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if let Some(completion_state) = &state.completion_state {
            if completion_state.candidates.is_empty() {
                return;
            }

            let max_items = 10;
            let scroll_offset = completion_state.scroll_offset;
            let end_idx = (scroll_offset + max_items).min(completion_state.candidates.len());
            let visible_candidates = &completion_state.candidates[scroll_offset..end_idx];

            let items: Vec<ListItem> = visible_candidates
                .iter()
                .enumerate()
                .map(|(idx, candidate)| {
                    let actual_idx = scroll_offset + idx;
                    let is_selected = actual_idx == completion_state.selected_index;
                    let style = if is_selected {
                        Style::default()
                            .fg(palette::SELECTED_FG)
                            .bg(palette::SELECTED_BG)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::PRIMARY_TEXT)
                    };

                    let prefix = if is_selected { "> " } else { "  " };
                    ListItem::new(format!("{}{}", prefix, candidate)).style(style)
                })
                .collect();

            let current_selection = completion_state.selected_index + 1;
            let total_completions = completion_state.candidates.len();
            let title = Span::styled(
                format!(" Files ( {} / {} ) ", current_selection, total_completions),
                Style::default()
                    .fg(palette::PRIMARY_BORDER)
                    .add_modifier(Modifier::BOLD),
            );

            // Clear the area first to prevent text bleed-through
            Clear.render(area, buf);

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette::PRIMARY_BORDER));

            let list = List::new(items).block(block);
            list.render(area, buf);
        }
    }
}
