/// Live token-by-token streaming buffer for the in-progress assistant reply.
#[derive(Default)]
pub struct StreamingState {
    pub text: Option<String>,
    /// Number of fully-rendered lines already flushed to scrollback.
    pub committed: usize,
    /// Set when the buffer holds the final text and should be flushed in full.
    pub finalize: bool,
    /// Append committed lines to scrollback (inline mode) vs. overlay (fullview).
    pub to_scrollback: bool,
}

impl StreamingState {
    pub(crate) fn start(&mut self) {
        self.text = Some(String::new());
        self.committed = 0;
        self.finalize = false;
    }

    pub(crate) fn push_delta(&mut self, delta: &str) {
        self.text.get_or_insert_with(String::new).push_str(delta);
    }

    pub(crate) fn replace_final(&mut self, content: String) {
        self.text = Some(content);
        self.finalize = true;
    }

    pub fn is_active(&self) -> bool {
        self.text.is_some()
    }

    pub fn visible_text(&self) -> Option<&str> {
        let buf = self.text.as_deref()?;
        if buf.trim().is_empty() {
            None
        } else {
            Some(buf)
        }
    }
}
