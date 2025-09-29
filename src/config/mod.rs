use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use crate::console::VerbosityLevel;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BackendConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub default_backend: String,
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub verbosity: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_prompt_path = Self::default_system_prompt_path()
            .ok()
            .and_then(|p| p.to_str().map(String::from));

        Self {
            default_backend: "mock".to_string(),
            backends: HashMap::new(),
            verbosity: None,
            system_prompt: default_prompt_path,
        }
    }
}

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant with access to tools for file operations and bash commands.

# Tool Usage Guidelines

## When to Use Tools
- Use tools when you need to read, write, or analyze files
- Use tools when you need to execute commands or check system state
- Use tools to gather information before providing answers

## When to Respond Directly
- After gathering necessary information with tools, provide your answer in a text response
- When answering questions that don't require file access or command execution
- When the user's request is complete and you're ready to hand control back

## Important Behavior Rules
1. **Always finish with a text response**: After using tools, analyze the results and provide a clear text response to the user
2. **Don't loop indefinitely**: Once you have enough information to answer the user's question, stop using tools and respond
3. **Be concise**: Provide clear, direct answers without unnecessary tool calls
4. **Return control**: When your task is complete, respond with text (no tool calls) so the user can provide their next instruction

## Example Flow
User: "What's in the README file?"
1. Use read_file tool to read README.md
2. Respond with text summarizing the contents (no more tool calls)

User: "Create a hello world program"
1. Use write_file tool to create the file
2. Respond with text confirming completion (no more tool calls)

Remember: Your goal is to help efficiently and then return control to the user by ending with a text-only response."#;

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

        Self::initialize_default_prompt()?;
        Ok(config)
    }

    fn initialize_default_prompt() -> Result<()> {
        let prompt_path = Self::default_system_prompt_path()?;
        if !prompt_path.exists() {
            fs::write(&prompt_path, DEFAULT_SYSTEM_PROMPT)
                .context("Failed to write default system prompt")?;
        }
        Ok(())
    }

    fn default_system_prompt_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("hoosh");

        fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;

        Ok(config_dir.join("system_prompt.txt"))
    }

    pub fn load_system_prompt(&self) -> Result<Option<String>> {
        if let Some(ref path) = self.system_prompt {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                let content = fs::read_to_string(&path_buf)
                    .with_context(|| format!("Failed to read system prompt from: {}", path))?;
                Ok(Some(content))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
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
            });

        match key {
            "api_key" => config.api_key = Some(value),
            "model" => config.model = Some(value),
            "base_url" => config.base_url = Some(value),
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
