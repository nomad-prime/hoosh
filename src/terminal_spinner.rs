use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time;

const BRAILLE_SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct TerminalSpinner {
    message: String,
    running: Arc<AtomicBool>,
}

impl TerminalSpinner {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&mut self) {
        self.running.store(true, Ordering::SeqCst);
        let message = self.message.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut frame = 0;
            while running.load(Ordering::SeqCst) {
                let spinner_char = BRAILLE_SPINNER[frame % BRAILLE_SPINNER.len()];
                eprint!("\r{} {}", spinner_char, message);
                let _ = io::stderr().flush();

                frame += 1;
                time::sleep(Duration::from_millis(80)).await;
            }

            eprint!("\r\x1b[2K");
            let _ = io::stderr().flush();
        });
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn update_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }
}

impl Drop for TerminalSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}
