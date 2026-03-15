use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

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
        }
    }
}
