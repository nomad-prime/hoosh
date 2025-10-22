use std::sync::{Arc, OnceLock};

/// Verbosity levels for console output
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum VerbosityLevel {
    /// Only show errors
    Quiet = 0,
    /// Normal output (default)
    #[default]
    Normal = 1,
    /// Verbose output with additional info
    Verbose = 2,
    /// Debug output with detailed information
    Debug = 3,
}

impl std::fmt::Display for VerbosityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerbosityLevel::Quiet => write!(f, "quiet"),
            VerbosityLevel::Normal => write!(f, "normal"),
            VerbosityLevel::Verbose => write!(f, "verbose"),
            VerbosityLevel::Debug => write!(f, "debug"),
        }
    }
}

/// Simple output utilities for non-interactive CLI commands.
/// Interactive chat uses the TUI system.
#[derive(Debug, Clone)]
pub struct Console {
    verbosity: VerbosityLevel,
}

impl Console {
    pub fn new(verbosity: VerbosityLevel) -> Self {
        Self { verbosity }
    }

    pub fn set_verbosity(&mut self, verbosity: VerbosityLevel) {
        self.verbosity = verbosity;
    }

    pub fn verbosity(&self) -> VerbosityLevel {
        self.verbosity
    }

    fn should_show(&self, level: VerbosityLevel) -> bool {
        self.verbosity >= level
    }

    pub fn error(&self, message: &str) {
        if self.verbosity > VerbosityLevel::Quiet {
            eprintln!("❌ {}", message);
        }
    }

    pub fn warning(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("⚠️  {}", message);
        }
    }

    pub fn info(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("ℹ️  {}", message);
        }
    }

    pub fn success(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("✅ {}", message);
        }
    }

    pub fn verbose(&self, message: &str) {
        if self.should_show(VerbosityLevel::Verbose) {
            println!("{}", message);
        }
    }

    pub fn debug(&self, message: &str) {
        if self.should_show(VerbosityLevel::Debug) {
            println!("🐛 DEBUG: {}", message);
        }
    }

    pub fn plain(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            println!("{}", message);
        }
    }

    pub fn print(&self, message: &str) {
        if self.should_show(VerbosityLevel::Normal) {
            print!("{}", message);
        }
    }

    pub fn newline(&self) {
        if self.should_show(VerbosityLevel::Normal) {
            println!();
        }
    }
}

static GLOBAL_CONSOLE: OnceLock<Arc<Console>> = OnceLock::new();

pub fn init_console(verbosity: VerbosityLevel) {
    let _ = GLOBAL_CONSOLE.set(Arc::new(Console::new(verbosity)));
}

pub fn console() -> Arc<Console> {
    GLOBAL_CONSOLE
        .get()
        .expect("Console not initialized - call init_console() first")
        .clone()
}

impl Default for Console {
    fn default() -> Self {
        Self {
            verbosity: VerbosityLevel::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verbosity_levels() {
        assert!(VerbosityLevel::Quiet < VerbosityLevel::Normal);
        assert!(VerbosityLevel::Normal < VerbosityLevel::Verbose);
        assert!(VerbosityLevel::Verbose < VerbosityLevel::Debug);
    }

    #[test]
    fn test_console_should_show() {
        let console = Console::new(VerbosityLevel::Normal);

        assert!(!console.should_show(VerbosityLevel::Verbose));
        assert!(console.should_show(VerbosityLevel::Normal));
        assert!(!console.should_show(VerbosityLevel::Debug));
    }

    #[test]
    fn test_verbosity_display() {
        assert_eq!(VerbosityLevel::Quiet.to_string(), "quiet");
        assert_eq!(VerbosityLevel::Normal.to_string(), "normal");
        assert_eq!(VerbosityLevel::Verbose.to_string(), "verbose");
        assert_eq!(VerbosityLevel::Debug.to_string(), "debug");
    }

    #[test]
    fn test_console_methods() {
        let console = Console::new(VerbosityLevel::Normal);

        assert_eq!(console.verbosity(), VerbosityLevel::Normal);

        let mut console = console;
        console.set_verbosity(VerbosityLevel::Verbose);
        assert_eq!(console.verbosity(), VerbosityLevel::Verbose);
    }
}
