use crate::console::VerbosityLevel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BackendConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentConfig {
    pub file: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub default_backend: String,
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub verbosity: Option<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default = "default_review_mode")]
    pub review_mode: bool,
}

fn default_review_mode() -> bool {
    true // Default to review mode (safer)
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut agents = HashMap::new();
        agents.insert(
            "assistant".to_string(),
            AgentConfig {
                file: "assistant.txt".to_string(),
                description: Some("General purpose assistant".to_string()),
                tags: vec![],
            },
        );

        Self {
            default_backend: "mock".to_string(),
            backends: HashMap::new(),
            verbosity: None,
            default_agent: Some("assistant".to_string()),
            agents,
            review_mode: default_review_mode(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path).context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")?
        } else {
            let config = Self::default();
            config.save()?;
            config
        };

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&config_path, content).context("Failed to write config file")
    }

    pub fn get_backend_config(&self, backend_name: &str) -> Option<&BackendConfig> {
        self.backends.get(backend_name)
    }

    pub fn set_backend_config(&mut self, backend_name: String, config: BackendConfig) {
        self.backends.insert(backend_name, config);
    }

    pub fn update_backend_setting(
        &mut self,
        backend_name: &str,
        key: &str,
        value: String,
    ) -> Result<()> {
        let config = self
            .backends
            .entry(backend_name.to_string())
            .or_insert_with(|| BackendConfig {
                api_key: None,
                model: None,
                base_url: None,
                temperature: None,
            });

        match key {
            "api_key" => config.api_key = Some(value),
            "model" => config.model = Some(value),
            "base_url" => config.base_url = Some(value),
            "temperature" => {
                let temp: f32 = value
                    .parse()
                    .context("Temperature must be a valid number")?;
                config.temperature = Some(temp);
            }
            _ => anyhow::bail!("Unknown backend config key: {}", key),
        }

        Ok(())
    }

    /// Get the configured verbosity level, falling back to Normal if not set
    pub fn get_verbosity(&self) -> VerbosityLevel {
        self.verbosity
            .as_ref()
            .and_then(|v| match v.as_str() {
                "quiet" => Some(VerbosityLevel::Quiet),
                "normal" => Some(VerbosityLevel::Normal),
                "verbose" => Some(VerbosityLevel::Verbose),
                "debug" => Some(VerbosityLevel::Debug),
                _ => None,
            })
            .unwrap_or(VerbosityLevel::Normal)
    }

    /// Set the verbosity level in configuration
    pub fn set_verbosity(&mut self, verbosity: VerbosityLevel) {
        self.verbosity = Some(verbosity.to_string());
    }

    fn config_path() -> Result<PathBuf> {
        let path = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Failed to get home directory")?;
        let mut path = PathBuf::from(path);
        path.push(".config");
        path.push("hoosh");
        path.push("config.toml");
        Ok(path)
    }
}
