use std::time::Instant;

const REVEAL_MIN_CPS: f32 = 40.0;
const REVEAL_DRAIN_SECS: f32 = 0.12;

/// Live token-by-token streaming buffer for the in-progress assistant reply.
pub struct StreamingState {
    pub text: Option<String>,
    /// Number of fully-rendered lines already flushed to scrollback.
    pub committed: usize,
    /// Set when the buffer holds the final text and should be flushed in full.
    pub finalize: bool,
    /// Append committed lines to scrollback (inline mode) vs. overlay (fullview).
    pub to_scrollback: bool,
    revealed: usize,
    last_reveal: Instant,
}

impl Default for StreamingState {
    fn default() -> Self {
        Self {
            text: None,
            committed: 0,
            finalize: false,
            to_scrollback: false,
            revealed: 0,
            last_reveal: Instant::now(),
        }
    }
}

impl StreamingState {
    pub(crate) fn start(&mut self) {
        self.text = Some(String::new());
        self.committed = 0;
        self.finalize = false;
        self.revealed = 0;
        self.last_reveal = Instant::now();
    }

    pub(crate) fn push_delta(&mut self, delta: &str) {
        self.text.get_or_insert_with(String::new).push_str(delta);
    }

    pub(crate) fn replace_final(&mut self, content: String) {
        self.text = Some(content);
        self.finalize = true;
    }

    pub(crate) fn advance_reveal(&mut self) {
        let Some(total) = self.text.as_deref().map(|t| t.chars().count()) else {
            self.last_reveal = Instant::now();
            return;
        };
        if self.finalize {
            self.revealed = total;
            return;
        }
        if self.revealed >= total {
            self.last_reveal = Instant::now();
            return;
        }
        let dt = self.last_reveal.elapsed().as_secs_f32();
        let gap = (total - self.revealed) as f32;
        let cps = (gap / REVEAL_DRAIN_SECS).max(REVEAL_MIN_CPS);
        let step = (cps * dt) as usize;
        if step == 0 {
            return;
        }
        self.revealed = (self.revealed + step).min(total);
        self.last_reveal = Instant::now();
    }

    pub(crate) fn revealed_slice(&self) -> &str {
        let Some(buf) = self.text.as_deref() else {
            return "";
        };
        let end = buf
            .char_indices()
            .nth(self.revealed)
            .map(|(i, _)| i)
            .unwrap_or(buf.len());
        &buf[..end]
    }

    pub(crate) fn revealed_complete(&self) -> bool {
        match self.text.as_deref() {
            Some(buf) => self.revealed >= buf.chars().count(),
            None => true,
        }
    }

    pub fn is_active(&self) -> bool {
        self.text.is_some()
    }

    pub fn visible_text(&self) -> Option<&str> {
        let slice = self.revealed_slice();
        if slice.trim().is_empty() {
            None
        } else {
            Some(slice)
        }
    }

    #[cfg(test)]
    pub(crate) fn reveal_all(&mut self) {
        if let Some(buf) = self.text.as_deref() {
            self.revealed = buf.chars().count();
        }
    }
}
