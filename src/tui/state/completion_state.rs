pub struct CompletionState {
    pub candidates: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub query: String,
    pub completer_index: usize,
}

impl CompletionState {
    pub fn new(completer_index: usize) -> Self {
        Self {
            candidates: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            query: String::new(),
            completer_index,
        }
    }

    pub fn selected_item(&self) -> Option<&str> {
        self.candidates.get(self.selected_index).map(|s| s.as_str())
    }

    pub fn select_next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.candidates.len();
            self.update_scroll_offset(10);
        }
    }

    pub fn select_prev(&mut self) {
        if !self.candidates.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.candidates.len() - 1;
            } else {
                self.selected_index -= 1;
            }
            self.update_scroll_offset(10);
        }
    }

    fn update_scroll_offset(&mut self, visible_items: usize) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_items {
            self.scroll_offset = self.selected_index.saturating_sub(visible_items - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_state_new_initializes_correctly() {
        let state = CompletionState::new(0);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
        assert!(state.candidates.is_empty());
        assert!(state.query.is_empty());
        assert_eq!(state.completer_index, 0);
    }

    #[test]
    fn completion_state_selected_item_returns_none_when_empty() {
        let state = CompletionState::new(0);
        assert_eq!(state.selected_item(), None);
    }

    #[test]
    fn completion_state_selected_item_returns_correct_item() {
        let mut state = CompletionState::new(0);
        state.candidates = vec!["foo".to_string(), "bar".to_string()];
        assert_eq!(state.selected_item(), Some("foo"));

        state.selected_index = 1;
        assert_eq!(state.selected_item(), Some("bar"));
    }

    #[test]
    fn completion_state_select_next_wraps_around() {
        let mut state = CompletionState::new(0);
        state.candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        state.select_next();
        assert_eq!(state.selected_index, 1);

        state.select_next();
        assert_eq!(state.selected_index, 2);

        state.select_next();
        assert_eq!(state.selected_index, 0); // wraps
    }

    #[test]
    fn completion_state_select_prev_wraps_around() {
        let mut state = CompletionState::new(0);
        state.candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        state.selected_index = 0;

        state.select_prev();
        assert_eq!(state.selected_index, 2); // wraps to end

        state.select_prev();
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn completion_state_select_next_empty_candidates() {
        let mut state = CompletionState::new(0);
        state.select_next();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn completion_state_scroll_offset_updates_when_scrolling() {
        let mut state = CompletionState::new(0);
        state.candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        state.selected_index = 2;

        state.update_scroll_offset(10);
        assert_eq!(state.scroll_offset, 0);

        // Test when selected_index would be out of view
        state.selected_index = 15;
        state.scroll_offset = 0;
        state.update_scroll_offset(10);
        assert_eq!(state.scroll_offset, 6); // 15 - (10 - 1) = 6
    }
}
