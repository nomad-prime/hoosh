use crate::config::{AgentConfig, AppConfig};
use crate::console;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub name: String,
    #[serde(skip)]
    pub content: String,
    pub file: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip)]
    pub core_instructions: String,
}

pub struct AgentDefinitionManager {
    config: AppConfig,
}

impl AgentDefinition {
    pub fn from_config(
        name: String,
        config: AgentConfig,
        content: String,
        core_instructions: String,
    ) -> Self {
        Self {
            name,
            content,
            file: config.file,
            description: config.description,
            tags: config.tags,
            core_instructions,
        }
    }
}

impl AgentDefinitionManager {
    pub fn new() -> Result<Self> {
        let config = AppConfig::load()?;
        Ok(Self { config })
    }

    pub fn initialize_default_agents(agents_dir: &Path) -> Result<()> {
        let default_prompts = [
            (
                "hoosh_planner.txt",
                include_str!("../prompts/hoosh_planner.txt"),
            ),
            (
                "hoosh_coder.txt",
                include_str!("../prompts/hoosh_coder.txt"),
            ),
            (
                "hoosh_reviewer.txt",
                include_str!("../prompts/hoosh_reviewer.txt"),
            ),
            (
                "hoosh_troubleshooter.txt",
                include_str!("../prompts/hoosh_troubleshooter.txt"),
            ),
            (
                "hoosh_assistant.txt",
                include_str!("../prompts/hoosh_assistant.txt"),
            ),
            (
                "hoosh_core_instructions.txt",
                include_str!("../prompts/hoosh_core_instructions.txt"),
            ),
            (
                "hoosh_coder_core_instructions.txt",
                include_str!("../prompts/hoosh_coder_core_instructions.txt"),
            ),
            (
                "hoosh_planner_core_instructions.txt",
                include_str!("../prompts/hoosh_planner_core_instructions.txt"),
            ),
            (
                "hoosh_reviewer_core_instructions.txt",
                include_str!("../prompts/hoosh_reviewer_core_instructions.txt"),
            ),
            (
                "hoosh_troubleshooter_core_instructions.txt",
                include_str!("../prompts/hoosh_troubleshooter_core_instructions.txt"),
            ),
            (
                "hoosh_assistant_core_instructions.txt",
                include_str!("../prompts/hoosh_assistant_core_instructions.txt"),
            ),
        ];

        for (file_name, content) in default_prompts {
            let agent_path = agents_dir.join(file_name);
            fs::write(&agent_path, content)
                .with_context(|| format!("Failed to write agent file: {}", file_name))?;
        }

        Ok(())
    }

    fn load_agent_content(&self, agent_config: &AgentConfig) -> Result<String> {
        let agents_dir = AppConfig::agents_dir()?;
        let agent_path = agents_dir.join(&agent_config.file);
        fs::read_to_string(&agent_path)
            .with_context(|| format!("Failed to read agent file: {}", agent_config.file))
    }

    pub fn get_agent(&self, name: &str) -> Option<AgentDefinition> {
        // Check custom agents first
        if let Some(agent_config) = self.config.agents.get(name) {
            return self.load_agent_content(agent_config).ok().map(|content| {
                let core_instructions = self
                    .config
                    .load_core_instructions(Some(name))
                    .unwrap_or_else(|_| "Focus on completing the task efficiently.".to_string());
                AgentDefinition::from_config(
                    name.to_string(),
                    agent_config.clone(),
                    content,
                    core_instructions,
                )
            });
        }

        None
    }

    pub fn get_default_agent(&self) -> Option<AgentDefinition> {
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

    pub fn list_agents(&self) -> Vec<AgentDefinition> {
        self.config
            .agents
            .iter()
            .filter_map(|(name, agent_config)| {
                self.load_agent_content(agent_config).ok().map(|content| {
                    let core_instructions = self
                        .config
                        .load_core_instructions(Some(name))
                        .unwrap_or_else(|_| {
                            "Focus on completing the task efficiently.".to_string()
                        });
                    AgentDefinition::from_config(
                        name.clone(),
                        agent_config.clone(),
                        content,
                        core_instructions,
                    )
                })
            })
            .collect()
    }
}
