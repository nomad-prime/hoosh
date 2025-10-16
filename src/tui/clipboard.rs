use anyhow::Result;
use arboard::Clipboard;

pub struct ClipboardManager {
    clipboard: Option<Clipboard>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        Self { clipboard }
    }

    pub fn get_text(&mut self) -> Result<String> {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard
                .get_text()
                .map_err(|e| anyhow::anyhow!("Failed to get clipboard text: {}", e))
        } else {
            Err(anyhow::anyhow!("Clipboard not available"))
        }
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
