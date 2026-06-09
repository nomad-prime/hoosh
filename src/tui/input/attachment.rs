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

/// An image pasted from the clipboard, sitting in the input draft until the
/// user submits it. PNG-encoded so backends can ship it directly.
#[derive(Clone, Debug)]
pub struct ImageAttachment {
    pub id: usize,
    pub media_type: String,
    pub data: Vec<u8>,
}

impl ImageAttachment {
    pub fn new(id: usize, media_type: String, data: Vec<u8>) -> Self {
        Self {
            id,
            media_type,
            data,
        }
    }
}
