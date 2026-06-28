use crate::tui::input::{ImageAttachment, TextAttachment};

/// Text and image attachments queued on the current draft, each with a
/// monotonic id used to match `[attachment-N]` / `[pasted image-N]` markers.
pub struct AttachmentState {
    pub text: Vec<TextAttachment>,
    pub images: Vec<ImageAttachment>,
    pub next_text_id: usize,
    pub next_image_id: usize,
}

impl Default for AttachmentState {
    fn default() -> Self {
        Self {
            text: Vec::new(),
            images: Vec::new(),
            next_text_id: 1,
            next_image_id: 1,
        }
    }
}

impl AttachmentState {
    pub fn add_text(&mut self, content: String) -> usize {
        let id = self.next_text_id;
        self.next_text_id += 1;
        self.text.push(TextAttachment::new(id, content));
        id
    }

    pub fn add_image(&mut self, media_type: String, data: Vec<u8>) -> usize {
        let id = self.next_image_id;
        self.next_image_id += 1;
        self.images.push(ImageAttachment::new(id, media_type, data));
        id
    }

    pub fn delete_text(&mut self, id: usize) -> bool {
        if let Some(index) = self.text.iter().position(|a| a.id == id) {
            self.text.remove(index);
            true
        } else {
            false
        }
    }

    pub fn get_text(&self, id: usize) -> Option<&TextAttachment> {
        self.text.iter().find(|a| a.id == id)
    }

    pub fn get_text_mut(&mut self, id: usize) -> Option<&mut TextAttachment> {
        self.text.iter_mut().find(|a| a.id == id)
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.images.clear();
        self.next_text_id = 1;
        self.next_image_id = 1;
    }

    pub fn drain_images(&mut self) -> Vec<crate::agent::Attachment> {
        let out = self
            .images
            .drain(..)
            .map(|att| crate::agent::Attachment {
                kind: crate::agent::AttachmentKind::Image,
                media_type: att.media_type,
                data: att.data,
            })
            .collect();
        self.next_image_id = 1;
        out
    }
}
