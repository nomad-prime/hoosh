use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

fn default_mention_handle() -> String {
    "@hoosh".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GithubConfig {
    #[serde(default)]
    pub webhook_secret: Option<String>,
    #[serde(default = "default_mention_handle")]
    pub mention_handle: String,
    #[serde(default)]
    pub bot_login: Option<String>,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            webhook_secret: None,
            mention_handle: default_mention_handle(),
            bot_login: None,
        }
    }
}

impl GithubConfig {
    pub fn startup_warnings(&self) -> Vec<&'static str> {
        let mut w = vec![];
        if self.webhook_secret.is_none() {
            w.push(
                "github.webhook_secret is not configured; webhook endpoint will return 500 until set",
            );
        }
        if self.bot_login.is_none() {
            w.push("github.bot_login is not configured; self-trigger protection is disabled");
        }
        w
    }
}

fn default_bind_address() -> SocketAddr {
    "127.0.0.1:7979".parse().unwrap()
}

fn default_token_budget() -> usize {
    100_000
}

fn default_sandbox_base_dir() -> PathBuf {
    std::env::temp_dir()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DaemonConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: SocketAddr,
    #[serde(default = "default_token_budget")]
    pub default_token_budget: usize,
    #[serde(default)]
    pub github_pat: Option<String>,
    #[serde(default)]
    pub ssh_key_path: Option<PathBuf>,
    #[serde(default = "default_sandbox_base_dir")]
    pub sandbox_base_dir: PathBuf,
    #[serde(default)]
    pub retain_sandboxes: bool,
    #[serde(default)]
    pub github: GithubConfig,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            default_token_budget: default_token_budget(),
            github_pat: None,
            ssh_key_path: None,
            sandbox_base_dir: default_sandbox_base_dir(),
            retain_sandboxes: false,
            github: GithubConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_warnings_emitted_when_webhook_secret_none() {
        let config = GithubConfig {
            webhook_secret: None,
            bot_login: Some("hoosh-bot".to_string()),
            ..Default::default()
        };
        let warnings = config.startup_warnings();
        assert!(
            warnings.iter().any(|w| w.contains("webhook_secret")),
            "Expected warning about webhook_secret, got: {:?}",
            warnings
        );
    }

    #[test]
    fn startup_warnings_emitted_when_bot_login_none() {
        let config = GithubConfig {
            webhook_secret: Some("secret".to_string()),
            bot_login: None,
            ..Default::default()
        };
        let warnings = config.startup_warnings();
        assert!(
            warnings.iter().any(|w| w.contains("bot_login")),
            "Expected warning about bot_login, got: {:?}",
            warnings
        );
    }

    #[test]
    fn no_startup_warnings_when_both_configured() {
        let config = GithubConfig {
            webhook_secret: Some("secret".to_string()),
            bot_login: Some("hoosh-bot".to_string()),
            ..Default::default()
        };
        let warnings = config.startup_warnings();
        assert!(
            warnings.is_empty(),
            "Expected no warnings, got: {:?}",
            warnings
        );
    }
}
