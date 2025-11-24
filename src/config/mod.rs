use crate::console::VerbosityLevel;
use crate::context_management::ContextManagerConfig;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

pub mod error;
pub use error::{ConfigError, ConfigResult};

pub const DEFAULT_AGENT_FILES: &[&str] = &[
    "hoosh_planner.txt",
    "hoosh_coder.txt",
    "hoosh_reviewer.txt",
    "hoosh_troubleshooter.txt",
    "hoosh_assistant.txt",
];

pub const DEFAULT_CORE_INSTRUCTIONS_FILE: &str = "hoosh_core_instructions.txt";

#[cfg(test)]
mod mod_tests;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BackendConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub chat_api: Option<String>,
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
    #[serde(default)]
    pub context_manager: Option<ContextManagerConfig>,
    #[serde(default)]
    pub core_reminder_token_threshold: Option<usize>,
    #[serde(default)]
    pub core_instructions_file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub default_backend: Option<String>,
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub verbosity: Option<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub context_manager: Option<ContextManagerConfig>,
    #[serde(default)]
    pub core_reminder_token_threshold: Option<usize>,
    #[serde(default)]
    pub core_instructions_file: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut agents = HashMap::new();

        for file_name in DEFAULT_AGENT_FILES {
            let agent_name = file_name.strip_suffix(".txt").unwrap_or(file_name);
            agents.insert(
                agent_name.to_string(),
                AgentConfig {
                    file: file_name.to_string(),
                    description: None,
                    tags: vec![],
                },
            );
        }

        Self {
            default_backend: "mock".to_string(),
            backends: HashMap::new(),
            verbosity: None,
            default_agent: Some("hoosh_coder".to_string()),
            agents,
            context_manager: None,
            core_reminder_token_threshold: None,
            core_instructions_file: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> ConfigResult<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Err(ConfigError::NotFound { path: config_path });
        }

        Self::validate_permissions(&config_path)?;

        let content = fs::read_to_string(&config_path).map_err(ConfigError::IoError)?;
        let mut config: Self = toml::from_str(&content).map_err(ConfigError::InvalidToml)?;

        if let Ok(project_path) = Self::project_config_path()
            && project_path.exists()
        {
            Self::validate_permissions(&project_path)?;
            let project_content =
                fs::read_to_string(&project_path).map_err(ConfigError::IoError)?;
            let project_config: ProjectConfig =
                toml::from_str(&project_content).map_err(ConfigError::InvalidToml)?;
            config.merge(project_config);
        }

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> ConfigResult<()> {
        if let Some(default_agent) = &self.default_agent
            && !self.agents.contains_key(default_agent)
        {
            eprintln!(
                "Warning: Configured default agent '{}' not found in agents configuration",
                default_agent
            );
            if !self.agents.is_empty() {
                let available_agents: Vec<&str> = self.agents.keys().map(|s| s.as_str()).collect();
                eprintln!("Available agents: {}", available_agents.join(", "));
            }
        }
        Ok(())
    }

    /// Validate that the config file has secure permissions (0600)
    fn validate_permissions(config_path: &std::path::Path) -> ConfigResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            if config_path.exists() {
                let metadata =
                    std::fs::metadata(config_path).map_err(|_| ConfigError::PermissionDenied)?;

                let permissions = metadata.mode() & 0o777; // Mask to get only permission bits

                // Check if permissions are more permissive than 0600
                if permissions != 0o600 {
                    eprintln!(
                        "⚠️  Security Warning: Config file permissions are {:o} (should be 0600)",
                        permissions
                    );
                    eprintln!("Run: chmod 600 {}", config_path.display());
                }
            }
        }

        // On non-Unix systems, we don't perform permission validation
        // as the permission model is different

        Ok(())
    }

    pub fn save(&self) -> ConfigResult<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(ConfigError::IoError)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        fs::write(&config_path, content).map_err(ConfigError::IoError)?;

        // Set secure permissions on Unix systems (0600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            fs::set_permissions(&config_path, permissions).map_err(ConfigError::IoError)?;
        }

        Ok(())
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
    ) -> ConfigResult<()> {
        let config = self
            .backends
            .entry(backend_name.to_string())
            .or_insert_with(|| BackendConfig {
                api_key: None,
                model: None,
                base_url: None,
                chat_api: None,
                temperature: None,
            });

        match key {
            "api_key" => config.api_key = Some(value),
            "model" => config.model = Some(value),
            "base_url" => config.base_url = Some(value),
            "chat_api" => config.chat_api = Some(value),
            "temperature" => {
                let temp: f32 = value.parse().map_err(|_| ConfigError::InvalidValue {
                    field: "temperature".to_string(),
                    value,
                })?;
                config.temperature = Some(temp);
            }
            _ => {
                return Err(ConfigError::UnknownConfigKey {
                    key: key.to_string(),
                });
            }
        }

        Ok(())
    }

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

    pub fn set_verbosity(&mut self, verbosity: VerbosityLevel) {
        self.verbosity = Some(verbosity.to_string());
    }

    pub fn set_default_agent(&mut self, agent_name: String) {
        self.default_agent = Some(agent_name);
    }

    pub fn get_context_manager_config(&self) -> ContextManagerConfig {
        self.context_manager.clone().unwrap_or_default()
    }

    pub fn load_core_instructions(&self) -> ConfigResult<String> {
        if let Some(custom_file) = &self.core_instructions_file {
            let agents_dir = Self::agents_dir()?;
            let path = agents_dir.join(custom_file);
            return fs::read_to_string(&path)
                .map_err(ConfigError::IoError)
                .map(|s| s.trim().to_string());
        }

        Ok(include_str!("../prompts/hoosh_core_instructions.txt")
            .trim()
            .to_string())
    }

    pub fn agents_dir() -> ConfigResult<PathBuf> {
        let path = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| ConfigError::NoHomeDirectory)?;
        let mut path = PathBuf::from(path);
        path.push(".config");
        path.push("hoosh");
        path.push("agents");
        Ok(path)
    }

    pub fn get_core_reminder_token_threshold(&self) -> usize {
        self.core_reminder_token_threshold.unwrap_or(10000)
    }

    pub fn config_path() -> ConfigResult<PathBuf> {
        let path = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| ConfigError::NoHomeDirectory)?;
        let mut path = PathBuf::from(path);
        path.push(".config");
        path.push("hoosh");
        path.push("config.toml");
        Ok(path)
    }

    pub fn project_config_path() -> ConfigResult<PathBuf> {
        let mut path = std::env::current_dir().map_err(ConfigError::IoError)?;
        path.push(".hoosh");
        path.push("config.toml");
        Ok(path)
    }

    pub fn merge(&mut self, other: ProjectConfig) {
        for (key, value) in other.backends {
            self.backends.insert(key, value);
        }

        for (key, value) in other.agents {
            self.agents.insert(key, value);
        }

        if let Some(default_backend) = other.default_backend
            && !default_backend.is_empty()
        {
            self.default_backend = default_backend;
        }

        if other.verbosity.is_some() {
            self.verbosity = other.verbosity;
        }

        if other.default_agent.is_some() {
            self.default_agent = other.default_agent;
        }

        if other.context_manager.is_some() {
            self.context_manager = other.context_manager;
        }

        if other.core_reminder_token_threshold.is_some() {
            self.core_reminder_token_threshold = other.core_reminder_token_threshold;
        }

        if other.core_instructions_file.is_some() {
            self.core_instructions_file = other.core_instructions_file;
        }
    }

    pub fn ensure_project_config() -> ConfigResult<()> {
        let project_path = Self::project_config_path()?;

        if let Some(parent) = project_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(ConfigError::IoError)?;
        }

        if !project_path.exists() {
            let empty_config = "# Project-specific configuration\n\
                # This file overrides settings from ~/.config/hoosh/config.toml\n\
                # Only specify settings you want to override here\n\n";
            fs::write(&project_path, empty_config).map_err(ConfigError::IoError)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(0o600);
                fs::set_permissions(&project_path, permissions).map_err(ConfigError::IoError)?;
            }
        }

        Ok(())
    }
}
