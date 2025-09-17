use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPrompt {
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemPromptsConfig {
    pub prompts: HashMap<String, SystemPrompt>,
    pub default_prompt: Option<String>,
}

pub struct SystemPromptManager {
    config: SystemPromptsConfig,
    config_path: PathBuf,
}

impl SystemPrompt {
    pub fn new(name: String, content: String) -> Self {
        Self {
            name,
            content,
            description: None,
            tags: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

impl Default for SystemPromptsConfig {
    fn default() -> Self {
        let mut prompts = HashMap::new();

        // Add some default system prompts
        prompts.insert(
            "assistant".to_string(),
            SystemPrompt::new(
                "assistant".to_string(),
                "You are a helpful AI assistant. Provide clear, accurate, and concise responses.".to_string()
            ).with_description("General purpose assistant prompt".to_string())
        );

        prompts.insert(
            "code-reviewer".to_string(),
            SystemPrompt::new(
                "code-reviewer".to_string(),
                "You are an expert code reviewer. Focus on code quality, security, performance, and best practices. Provide constructive feedback.".to_string()
            ).with_description("Code review focused prompt".to_string())
            .with_tags(vec!["coding".to_string(), "review".to_string()])
        );

        prompts.insert(
            "rust-expert".to_string(),
            SystemPrompt::new(
                "rust-expert".to_string(),
                "You are a Rust programming expert. Help with Rust code, best practices, memory safety, and performance optimization.".to_string()
            ).with_description("Rust programming expert prompt".to_string())
            .with_tags(vec!["rust".to_string(), "programming".to_string()])
        );

        Self {
            prompts,
            default_prompt: Some("assistant".to_string()),
        }
    }
}

impl SystemPromptManager {
    pub fn new() -> Result<Self> {
        let config_path = Self::config_path()?;
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            let default_config = SystemPromptsConfig::default();
            Self::save_config(&config_path, &default_config)?;
            default_config
        };

        Ok(Self { config, config_path })
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("hoosh");

        fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;

        Ok(config_dir.join("system_prompts.toml"))
    }

    fn load_config(path: &PathBuf) -> Result<SystemPromptsConfig> {
        let content = fs::read_to_string(path)
            .context("Failed to read system prompts config file")?;
        toml::from_str(&content)
            .context("Failed to parse system prompts config")
    }

    fn save_config(path: &PathBuf, config: &SystemPromptsConfig) -> Result<()> {
        let content = toml::to_string_pretty(config)
            .context("Failed to serialize system prompts config")?;
        fs::write(path, content)
            .context("Failed to write system prompts config file")
    }

    pub fn get_prompt(&self, name: &str) -> Option<&SystemPrompt> {
        self.config.prompts.get(name)
    }

    pub fn get_default_prompt(&self) -> Option<&SystemPrompt> {
        self.config.default_prompt.as_ref()
            .and_then(|name| self.config.prompts.get(name))
    }

    pub fn list_prompts(&self) -> Vec<&SystemPrompt> {
        self.config.prompts.values().collect()
    }

    pub fn add_prompt(&mut self, prompt: SystemPrompt) -> Result<()> {
        self.config.prompts.insert(prompt.name.clone(), prompt);
        self.save()
    }

    pub fn remove_prompt(&mut self, name: &str) -> Result<bool> {
        let removed = self.config.prompts.remove(name).is_some();
        if removed {
            // If we removed the default prompt, clear the default
            if self.config.default_prompt.as_ref() == Some(&name.to_string()) {
                self.config.default_prompt = None;
            }
            self.save()?;
        }
        Ok(removed)
    }

    pub fn set_default_prompt(&mut self, name: &str) -> Result<bool> {
        if self.config.prompts.contains_key(name) {
            self.config.default_prompt = Some(name.to_string());
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn find_prompts_by_tag(&self, tag: &str) -> Vec<&SystemPrompt> {
        self.config.prompts.values()
            .filter(|prompt| prompt.tags.contains(&tag.to_string()))
            .collect()
    }

    fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }
}