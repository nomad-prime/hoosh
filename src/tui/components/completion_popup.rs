use crate::tui::app::AppState;
use ratatui::text::Span;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Widget},
};

/// Completion popup widget that shows file/command completion options
pub struct CompletionPopupWidget<'a> {
    app_state: &'a AppState,
    anchor_area: Rect,
}

impl<'a> CompletionPopupWidget<'a> {
    pub fn new(app_state: &'a AppState, anchor_area: Rect) -> Self {
        Self {
            app_state,
            anchor_area,
        }
    }
}

impl<'a> Widget for CompletionPopupWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let Some(completion_state) = &self.app_state.completion_state {
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
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let prefix = if is_selected { "> " } else { "  " };
                    ListItem::new(format!("{}{}", prefix, candidate)).style(style)
                })
                .collect();

            let viewport_area = area;
            let popup_start_y = self.anchor_area.y + self.anchor_area.height;
            let viewport_bottom = viewport_area.y + viewport_area.height;
            let available_height = viewport_bottom.saturating_sub(popup_start_y);
            let desired_height = visible_candidates.len() as u16 + 2;
            let popup_height = desired_height.min(available_height).max(3);

            let popup_width = visible_candidates
                .iter()
                .map(|c| c.len())
                .max()
                .unwrap_or(20)
                .min(60) as u16
                + 4;

            let width = popup_width.max(32);

            let popup_area = Rect {
                x: self.anchor_area.x,
                y: popup_start_y,
                width,
                height: popup_height,
            };

            let current_selection = completion_state.selected_index + 1;
            let total_completions = completion_state.candidates.len();
            let title = Span::styled(
                format!(" Files ( {} / {} ) ", current_selection, total_completions),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );

            // Clear the area first to prevent text bleed-through
            Clear.render(popup_area, buf);

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let list = List::new(items).block(block);
            list.render(popup_area, buf);
        }
    }
}
