use crate::console::VerbosityLevel;
use crate::context_management::ContextManagerConfig;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

pub mod error;
pub use error::{ConfigError, ConfigResult};

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
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut agents = HashMap::new();

        // Get the path to the prompts directory in the source code
        let prompts_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("prompts");

        // Read all files from the prompts directory
        if let Ok(prompt_files) = std::fs::read_dir(&prompts_dir) {
            for entry in prompt_files.filter_map(|e| e.ok()) {
                let file_path = entry.path();

                // Skip directories and non-file entries
                if !file_path.is_file() {
                    continue;
                }

                // Get the file name without extension for the agent name
                if let Some(file_name) = file_path.file_name().and_then(|f| f.to_str()) {
                    // Remove .txt extension for the agent name
                    let agent_name = if let Some(stripped) = file_name.strip_suffix(".txt") {
                        stripped.to_string()
                    } else {
                        file_name.to_string()
                    };

                    // Add the agent to the config
                    agents.insert(
                        agent_name,
                        AgentConfig {
                            file: file_name.to_string(),
                            description: None,
                            tags: vec![],
                        },
                    );
                }
            }
        }

        Self {
            default_backend: "mock".to_string(),
            backends: HashMap::new(),
            verbosity: None,
            default_agent: Some("hoosh_coder".to_string()),
            agents,
            context_manager: None,
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

        if let Ok(project_path) = Self::project_config_path() {
            if project_path.exists() {
                Self::validate_permissions(&project_path)?;
                let project_content =
                    fs::read_to_string(&project_path).map_err(ConfigError::IoError)?;
                let project_config: ProjectConfig =
                    toml::from_str(&project_content).map_err(ConfigError::InvalidToml)?;
                config.merge(project_config);
            }
        }

        // Ensure default agents are always available
        config.ensure_default_agents()?;

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

    /// Ensure default agents from prompts directory are available in config
    fn ensure_default_agents(&mut self) -> ConfigResult<()> {
        // Get the path to the prompts directory in the source code
        let prompts_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("prompts");

        // Read all files from the prompts directory
        if let Ok(prompt_files) = std::fs::read_dir(&prompts_dir) {
            for entry in prompt_files.filter_map(|e| e.ok()) {
                let file_path = entry.path();

                // Skip directories and non-file entries
                if !file_path.is_file() {
                    continue;
                }

                // Get the file name without extension for the agent name
                if let Some(file_name) = file_path.file_name().and_then(|f| f.to_str()) {
                    // Remove .txt extension for the agent name
                    let agent_name = if let Some(stripped) = file_name.strip_suffix(".txt") {
                        stripped.to_string()
                    } else {
                        file_name.to_string()
                    };

                    // Add the agent to the config if it doesn't exist
                    self.agents.entry(agent_name).or_insert(AgentConfig {
                        file: file_name.to_string(),
                        description: None,
                        tags: vec![],
                    });
                }
            }
        }

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

    /// Set the default agent in configuration
    pub fn set_default_agent(&mut self, agent_name: String) {
        self.default_agent = Some(agent_name);
    }

    /// Get the context manager configuration, or default if not set
    pub fn get_context_manager_config(&self) -> ContextManagerConfig {
        self.context_manager.clone().unwrap_or_default()
    }

    /// Set the context manager configuration
    pub fn set_context_manager_config(&mut self, config: ContextManagerConfig) {
        self.context_manager = Some(config);
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

        if let Some(default_backend) = other.default_backend {
            if !default_backend.is_empty() {
                self.default_backend = default_backend;
            }
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
    }

    pub fn ensure_project_config() -> ConfigResult<()> {
        let project_path = Self::project_config_path()?;

        if let Some(parent) = project_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(ConfigError::IoError)?;
            }
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
