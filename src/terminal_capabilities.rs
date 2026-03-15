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
    pub fn detect() -> Result<Self> {
        Ok(Self::from_env(
            std::env::var("TERM_PROGRAM").ok().as_deref(),
            std::env::var("COLORTERM").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
            std::env::var("VSCODE_GIT_IPC_HANDLE").is_ok(),
            std::env::var("VSCODE_INJECTION").is_ok(),
        ))
    }

    fn from_env(
        term_program: Option<&str>,
        colorterm: Option<&str>,
        term: Option<&str>,
        has_vscode_ipc: bool,
        has_vscode_injection: bool,
    ) -> Self {
        let is_vscode =
            term_program == Some("vscode") || has_vscode_ipc || has_vscode_injection;

        let is_iterm = term_program == Some("iTerm.app");

        let supports_mouse = !matches!(term, Some("dumb") | Some("unknown"));

        Self {
            supports_mouse,
            is_vscode,
            is_iterm,
            term_program: term_program.map(str::to_owned),
            colorterm: colorterm.map(str::to_owned),
        }
    }

    pub fn warn_if_vscode_with_inline(&self, mode: crate::terminal_mode::TerminalMode) {
        if self.is_vscode && mode == crate::terminal_mode::TerminalMode::Inline {
            eprintln!("⚠️  VSCode terminal detected with inline mode.");
            eprintln!("   Consider using --mode fullview for better compatibility.");
            eprintln!("   Set terminal_mode = \"fullview\" in config to suppress this message.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_mode::TerminalMode;

    #[test]
    fn detects_vscode_via_term_program() {
        let caps = TerminalCapabilities::from_env(Some("vscode"), None, None, false, false);
        assert!(caps.is_vscode);
        assert_eq!(caps.term_program, Some("vscode".to_string()));
    }

    #[test]
    fn detects_vscode_via_ipc_handle() {
        let caps = TerminalCapabilities::from_env(None, None, None, true, false);
        assert!(caps.is_vscode);
    }

    #[test]
    fn detects_vscode_via_injection() {
        let caps = TerminalCapabilities::from_env(None, None, None, false, true);
        assert!(caps.is_vscode);
    }

    #[test]
    fn detects_iterm() {
        let caps = TerminalCapabilities::from_env(Some("iTerm.app"), None, None, false, false);
        assert!(caps.is_iterm);
        assert_eq!(caps.term_program, Some("iTerm.app".to_string()));
    }

    #[test]
    fn mouse_supported_by_default() {
        let caps = TerminalCapabilities::from_env(None, None, None, false, false);
        assert!(caps.supports_mouse);
    }

    #[test]
    fn no_mouse_on_dumb_terminal() {
        let caps = TerminalCapabilities::from_env(None, None, Some("dumb"), false, false);
        assert!(!caps.supports_mouse);
    }

    #[test]
    fn warn_vscode_with_inline_no_panic() {
        let caps = TerminalCapabilities::from_env(Some("vscode"), None, None, false, false);
        caps.warn_if_vscode_with_inline(TerminalMode::Inline);
    }
}
