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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn initialize_default_agents_writes_all_agent_files() {
        let dir = TempDir::new().unwrap();
        AgentDefinitionManager::initialize_default_agents(dir.path()).unwrap();

        for (file_name, _) in crate::config::DEFAULT_AGENTS {
            assert!(
                dir.path().join(file_name).exists(),
                "Missing agent file: {}",
                file_name
            );
        }
    }

    #[test]
    fn initialize_default_agents_writes_all_core_instruction_files() {
        let dir = TempDir::new().unwrap();
        AgentDefinitionManager::initialize_default_agents(dir.path()).unwrap();

        for (file_name, _) in crate::config::DEFAULT_CORE_INSTRUCTIONS {
            assert!(
                dir.path().join(file_name).exists(),
                "Missing core instructions file: {}",
                file_name
            );
        }
    }

    #[test]
    fn initialize_default_agents_writes_correct_content() {
        let dir = TempDir::new().unwrap();
        AgentDefinitionManager::initialize_default_agents(dir.path()).unwrap();

        for (file_name, expected_content) in crate::config::DEFAULT_AGENTS {
            let actual = std::fs::read_to_string(dir.path().join(file_name)).unwrap();
            assert_eq!(actual, *expected_content, "Content mismatch for: {}", file_name);
        }
    }

    #[test]
    fn default_config_registers_all_builtin_agents() {
        let config = AppConfig::default();
        for (file_name, _) in crate::config::DEFAULT_AGENTS {
            let agent_name = file_name.strip_suffix(".txt").unwrap_or(file_name);
            assert!(
                config.agents.contains_key(agent_name),
                "Default config missing entry for builtin agent: {}",
                agent_name
            );
        }
    }
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
        for (file_name, content) in crate::config::DEFAULT_AGENTS {
            let agent_path = agents_dir.join(file_name);
            fs::write(&agent_path, content)
                .with_context(|| format!("Failed to write agent file: {}", file_name))?;
        }

        for (file_name, content) in crate::config::DEFAULT_CORE_INSTRUCTIONS {
            let path = agents_dir.join(file_name);
            fs::write(&path, content)
                .with_context(|| format!("Failed to write core instructions: {}", file_name))?;
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
