#[derive(Debug, Clone, PartialEq)]
pub enum PasteClassification {
    Inline,
    Attachment,
    Rejected(String),
}

pub struct PasteDetector {
    threshold_chars: usize,
    max_size_bytes: usize,
}

impl PasteDetector {
    pub fn new() -> Self {
        Self {
            threshold_chars: 200,
            max_size_bytes: 5_000_000, // 5MB
        }
    }

    pub fn with_threshold(threshold_chars: usize, max_size_bytes: usize) -> Self {
        Self {
            threshold_chars,
            max_size_bytes,
        }
    }

    pub fn classify_paste(&self, content: &str) -> PasteClassification {
        let size_bytes = content.len();

        if size_bytes > self.max_size_bytes {
            return PasteClassification::Rejected(format!(
                "Paste rejected: exceeds {}MB limit",
                self.max_size_bytes / 1_000_000
            ));
        }

        let char_count = content.chars().count();

        if char_count > self.threshold_chars {
            PasteClassification::Attachment
        } else {
            PasteClassification::Inline
        }
    }
}

impl Default for PasteDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "paste_detector_tests.rs"]
mod tests;
