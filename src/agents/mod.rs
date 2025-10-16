use crate::config::{AgentConfig, AppConfig};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_ASSISTANT_PROMPT: &str = include_str!("../prompts/assistant.txt");
const HOOSH_CODER_PROMPT: &str = include_str!("../prompts/hoosh_coder.txt");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    #[serde(skip)]
    pub content: String,
    pub file: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

pub struct AgentManager {
    config: AppConfig,
}

impl Agent {
    pub fn from_config(name: String, config: AgentConfig, content: String) -> Self {
        Self {
            name,
            content,
            file: config.file,
            description: config.description,
            tags: config.tags,
        }
    }
}



impl AgentManager {
    pub fn new() -> Result<Self> {
        let config = AppConfig::load()?;
        let agents_dir = Self::agents_dir()?;
        Self::initialize_default_agents(&agents_dir)?;

        Ok(Self { config })
    }

    fn agents_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Failed to get home directory")?;
        let agents_dir = PathBuf::from(home)
            .join(".config")
            .join("hoosh")
            .join("agents");

        fs::create_dir_all(&agents_dir).context("Failed to create agents directory")?;

        Ok(agents_dir)
    }

    fn initialize_default_agents(agents_dir: &Path) -> Result<()> {
        let assistant_path = agents_dir.join("assistant.txt");
        if !assistant_path.exists() {
            fs::write(&assistant_path, DEFAULT_ASSISTANT_PROMPT)
                .context("Failed to write default assistant agent")?;
        }

        let hoosh_coder_path = agents_dir.join("hoosh_coder.txt");
        if !hoosh_coder_path.exists() {
            fs::write(&hoosh_coder_path, HOOSH_CODER_PROMPT)
                .context("Failed to write hoosh_coder agent")?;
        }

        Ok(())
    }

    fn load_agent_content(&self, agent_config: &AgentConfig) -> Result<String> {
        let agents_dir = Self::agents_dir()?;
        let agent_path = agents_dir.join(&agent_config.file);
        fs::read_to_string(&agent_path)
            .with_context(|| format!("Failed to read agent file: {}", agent_config.file))
    }

    pub fn get_agent(&self, name: &str) -> Option<Agent> {
        self.config.agents.get(name).and_then(|agent_config| {
            self.load_agent_content(agent_config)
                .ok()
                .map(|content| Agent::from_config(name.to_string(), agent_config.clone(), content))
        })
    }

    pub fn get_default_agent(&self) -> Option<Agent> {
        self.config
            .default_agent
            .as_ref()
            .and_then(|name| self.get_agent(name))
    }

    pub fn list_agents(&self) -> Vec<Agent> {
        self.config
            .agents
            .iter()
            .filter_map(|(name, agent_config)| {
                self.load_agent_content(agent_config)
                    .ok()
                    .map(|content| Agent::from_config(name.clone(), agent_config.clone(), content))
            })
            .collect()
    }
}
