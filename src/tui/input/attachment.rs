use std::time::Instant;

#[derive(Clone, Debug)]
pub struct TextAttachment {
    pub id: usize,
    pub content: String,
    pub size_chars: usize,
    pub line_count: usize,
    pub created_at: Instant,
}

impl TextAttachment {
    pub fn new(id: usize, content: String) -> Self {
        let size_chars = content.chars().count();
        let line_count = content.lines().count().max(1);

        Self {
            id,
            content,
            size_chars,
            line_count,
            created_at: Instant::now(),
        }
    }

    pub fn update_content(&mut self, content: String) {
        self.size_chars = content.chars().count();
        self.line_count = content.lines().count().max(1);
        self.content = content;
    }
}
