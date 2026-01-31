#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub content_height: usize,
    pub viewport_height: usize,
    pub viewport_width: usize,
    pub velocity: f32,
}

impl ScrollState {
    pub fn new(viewport_height: usize) -> Self {
        Self {
            offset: 0,
            content_height: 0,
            viewport_height,
            viewport_width: 80,
            velocity: 0.0,
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = (self.offset + lines).min(max_offset);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.content_height.saturating_sub(self.viewport_height);
    }

    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.content_height.saturating_sub(self.viewport_height)
    }

    pub fn page_down(&mut self) {
        self.scroll_down(self.viewport_height.saturating_sub(1));
    }

    pub fn page_up(&mut self) {
        self.scroll_up(self.viewport_height.saturating_sub(1));
    }

    pub fn update_viewport_height(&mut self, new_height: usize) {
        self.viewport_height = new_height;
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = self.offset.min(max_offset);
    }

    pub fn update_viewport_width(&mut self, new_width: usize) {
        self.viewport_width = new_width;
    }

    pub fn update_viewport_size(&mut self, width: usize, height: usize) {
        self.viewport_width = width;
        self.viewport_height = height;
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = self.offset.min(max_offset);
    }

    pub fn update_content_height(&mut self, new_height: usize) {
        self.content_height = new_height;
        let max_offset = self.content_height.saturating_sub(self.viewport_height);
        self.offset = self.offset.min(max_offset);
    }
}
