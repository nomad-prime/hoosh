use crate::tui::app::AppState;
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
            let visible_candidates =
                &completion_state.candidates[..completion_state.candidates.len().min(max_items)];

            let items: Vec<ListItem> = visible_candidates
                .iter()
                .enumerate()
                .map(|(idx, candidate)| {
                    let is_selected = idx == completion_state.selected_index;
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

            let popup_area = Rect {
                x: self.anchor_area.x,
                y: popup_start_y,
                width: popup_width,
                height: popup_height,
            };

            // Clear the area first to prevent text bleed-through
            Clear.render(popup_area, buf);

            let block = Block::default()
                .title(" File Completion ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let list = List::new(items).block(block);
            list.render(popup_area, buf);
        }
    }
}
