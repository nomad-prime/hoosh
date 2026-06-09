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

    pub fn set_text(&mut self, text: String) -> Result<()> {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard
                .set_text(text)
                .map_err(|e| anyhow::anyhow!("Failed to set clipboard text: {}", e))
        } else {
            Err(anyhow::anyhow!("Clipboard not available"))
        }
    }

    /// Read an image off the system clipboard and PNG-encode it. Returns
    /// `(png_bytes, "image/png")`. Returns Err when there is no image on the
    /// clipboard or the encoder fails.
    pub fn get_image_png(&mut self) -> Result<(Vec<u8>, &'static str)> {
        let clipboard = self
            .clipboard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Clipboard not available"))?;
        let img = clipboard
            .get_image()
            .map_err(|e| anyhow::anyhow!("No image on clipboard: {}", e))?;

        let mut out: Vec<u8> = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, img.width as u32, img.height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| anyhow::anyhow!("png header: {}", e))?;
            writer
                .write_image_data(&img.bytes)
                .map_err(|e| anyhow::anyhow!("png data: {}", e))?;
        }
        Ok((out, "image/png"))
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
