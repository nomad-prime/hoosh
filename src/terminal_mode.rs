// Terminal display mode enum

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Terminal display mode for hoosh sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TerminalMode {
    /// Default mode: grows dynamically with Viewport::Inline
    /// Works with native terminal scrollback
    #[default]
    Inline,

    /// Fullscreen mode: Viewport::Fullscreen with internal scrolling
    /// Compatible with VSCode terminals and broken scrollback environments
    Fullview,

    /// Non-hijacking mode: Terminal-native output, shell integration
    /// Uses @hoosh alias, session file persistence, returns control to shell
    Tagged,
}

impl FromStr for TerminalMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inline" => Ok(Self::Inline),
            "fullview" => Ok(Self::Fullview),
            "tagged" => Ok(Self::Tagged),
            _ => Err(anyhow!("Invalid terminal mode: {}", s)),
        }
    }
}

impl std::fmt::Display for TerminalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inline => write!(f, "inline"),
            Self::Fullview => write!(f, "fullview"),
            Self::Tagged => write!(f, "tagged"),
        }
    }
}

pub fn select_terminal_mode(cli_mode: Option<String>, config_mode: Option<String>) -> TerminalMode {
    if let Some(mode_str) = cli_mode
        && let Ok(mode) = mode_str.parse()
    {
        return mode;
    }

    if let Some(mode_str) = config_mode
        && let Ok(mode) = mode_str.parse()
    {
        return mode;
    }

    TerminalMode::default()
}
