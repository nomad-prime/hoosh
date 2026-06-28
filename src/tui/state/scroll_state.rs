use ratatui::widgets::ScrollbarState;

/// Vertical scrollback position and the geometry the scrollbar widget needs.
#[derive(Default)]
pub struct ScrollState {
    pub offset: usize,
    pub bar: ScrollbarState,
    pub content_length: usize,
    pub viewport_length: usize,
}

impl ScrollState {
    pub fn max_offset(&self) -> usize {
        self.content_length.saturating_sub(self.viewport_length)
    }

    pub fn at_bottom(&self) -> bool {
        self.offset >= self.max_offset()
    }

    pub fn page(&self) -> usize {
        self.viewport_length.saturating_sub(1)
    }

    pub fn half_page(&self) -> usize {
        self.viewport_length / 2
    }

    pub fn up(&mut self, lines: usize) {
        self.set_offset(self.offset.saturating_sub(lines));
    }

    pub fn down(&mut self, lines: usize) {
        self.set_offset(self.offset.saturating_add(lines));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.set_offset(self.max_offset());
    }

    /// Re-clamp the offset after the content or viewport size changed.
    pub fn clamp(&mut self) {
        self.set_offset(self.offset);
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = offset.min(self.max_offset());
        self.bar = self.bar.position(self.offset);
    }

    /// Push the current geometry into the scrollbar widget state.
    pub fn sync_bar(&mut self) {
        self.bar = self
            .bar
            .content_length(self.content_length)
            .viewport_content_length(self.viewport_length)
            .position(self.offset);
    }
}
