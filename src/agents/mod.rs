use crate::config::{AgentConfig, AppConfig};
use crate::console;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
        // Get the path to the prompts directory in the source code
        let prompts_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("prompts");

        // Read all files from the prompts directory
        let prompt_files =
            fs::read_dir(&prompts_dir).context("Failed to read prompts directory")?;

        // Process each prompt file
        for entry in prompt_files {
            let entry = entry.context("Failed to read directory entry")?;
            let file_path = entry.path();

            // Skip directories and non-file entries
            if !file_path.is_file() {
                continue;
            }

            // Get the file name
            let file_name = file_path
                .file_name()
                .context("Failed to get file name")?
                .to_str()
                .context("Failed to convert file name to string")?;

            // Read the prompt content
            let prompt_content = fs::read_to_string(&file_path)
                .with_context(|| format!("Failed to read prompt file: {}", file_name))?;

            // Write the prompt content to the agent file
            let agent_path = agents_dir.join(file_name);
            fs::write(&agent_path, prompt_content)
                .with_context(|| format!("Failed to write agent file: {}", file_name))?;
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
        // Check custom agents first
        if let Some(agent_config) = self.config.agents.get(name) {
            return self.load_agent_content(agent_config).ok().map(|content| {
                Agent::from_config(name.to_string(), agent_config.clone(), content)
            });
        }

        // Check default agents
        let default_agents = [
            "planner",
            "coder",
            "reviewer",
            "troubleshooter",
            "assistant",
        ];
        if default_agents.contains(&name) {
            let agents_dir = Self::agents_dir().ok()?;
            let file_name = format!("{}.txt", name);
            let agent_path = agents_dir.join(&file_name);
            let content = std::fs::read_to_string(&agent_path).ok()?;

            let description = match name {
                "planner" => Some(
                    "Use for planning, analysis, and breaking down complex problems".to_string(),
                ),
                "coder" => Some("Use for implementation, coding, and executing tasks".to_string()),
                "reviewer" => Some(
                    "Use for reviewing code, identifying issues, and providing feedback"
                        .to_string(),
                ),
                "troubleshooter" => {
                    Some("Use for debugging, troubleshooting, and fixing problems".to_string())
                }
                "assistant" => {
                    Some("General-purpose helpful assistant with access to tools".to_string())
                }
                _ => None,
            };

            let tags = match name {
                "planner" => vec![
                    "planning".to_string(),
                    "analysis".to_string(),
                    "design".to_string(),
                ],
                "coder" => vec![
                    "implementation".to_string(),
                    "coding".to_string(),
                    "execution".to_string(),
                ],
                "reviewer" => vec![
                    "review".to_string(),
                    "feedback".to_string(),
                    "quality".to_string(),
                ],
                "troubleshooter" => vec![
                    "debugging".to_string(),
                    "troubleshooting".to_string(),
                    "fixing".to_string(),
                ],
                "assistant" => vec![
                    "general".to_string(),
                    "assistance".to_string(),
                    "simple".to_string(),
                ],
                _ => vec![],
            };

            return Some(Agent {
                name: name.to_string(),
                content,
                file: file_name,
                description,
                tags,
            });
        }

        None
    }

    pub fn get_default_agent(&self) -> Option<Agent> {
        if let Some(name) = &self.config.default_agent {
            if let Some(agent) = self.get_agent(name) {
                return Some(agent);
            } else {
                let available_agents: Vec<&str> =
                    self.config.agents.keys().map(|s| s.as_str()).collect();
                console::console().warning(&format!(
                        "Configured default agent '{}' not found. Available agents: {}. Falling back to first available agent.",
                        name,
                        available_agents.join(", ")
                    ));
            }
        }

        // Fallback to the first available agent
        self.list_agents().into_iter().next()
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
