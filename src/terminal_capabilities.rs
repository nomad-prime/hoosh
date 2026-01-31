// Terminal capabilities detection

use anyhow::Result;

/// Terminal capabilities and environment detection
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    /// Mouse events supported (for fullview scrolling)
    pub supports_mouse: bool,

    /// Running in VSCode integrated terminal
    pub is_vscode: bool,

    /// Running in iTerm2
    pub is_iterm: bool,

    /// TERM_PROGRAM environment variable
    pub term_program: Option<String>,

    /// COLORTERM environment variable
    pub colorterm: Option<String>,
}

impl TerminalCapabilities {
    /// Detect terminal capabilities from environment
    pub fn detect() -> Result<Self> {
        let term_program = std::env::var("TERM_PROGRAM").ok();
        let colorterm = std::env::var("COLORTERM").ok();

        let is_vscode = term_program.as_deref() == Some("vscode")
            || std::env::var("VSCODE_GIT_IPC_HANDLE").is_ok()
            || std::env::var("VSCODE_INJECTION").is_ok();

        let is_iterm = term_program.as_deref() == Some("iTerm.app");

        // Mouse support check (most modern terminals support it)
        let supports_mouse = !matches!(
            std::env::var("TERM").ok().as_deref(),
            Some("dumb") | Some("unknown")
        );

        Ok(Self {
            supports_mouse,
            is_vscode,
            is_iterm,
            term_program,
            colorterm,
        })
    }

    /// Warn if VSCode detected but using inline mode
    pub fn warn_if_vscode_with_inline(&self, mode: crate::terminal_mode::TerminalMode) {
        if self.is_vscode && mode == crate::terminal_mode::TerminalMode::Inline {
            eprintln!("⚠️  VSCode terminal detected with inline mode.");
            eprintln!("   Consider using --mode fullview for better compatibility.");
            eprintln!("   Set terminal_mode = \"fullview\" in config to suppress this message.");
        }
    }
}
