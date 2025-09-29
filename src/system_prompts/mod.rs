use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPrompt {
    pub name: String,
    #[serde(skip)]
    pub content: String,
    pub file: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemPromptsConfig {
    pub prompts: HashMap<String, PromptMetadata>,
    pub default_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMetadata {
    pub file: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

pub struct SystemPromptManager {
    config: SystemPromptsConfig,
    config_path: PathBuf,
}

impl SystemPrompt {
    pub fn from_metadata(name: String, metadata: PromptMetadata, content: String) -> Self {
        Self {
            name,
            content,
            file: metadata.file,
            description: metadata.description,
            tags: metadata.tags,
        }
    }
}

impl Default for SystemPromptsConfig {
    fn default() -> Self {
        let mut prompts = HashMap::new();

        prompts.insert(
            "assistant".to_string(),
            PromptMetadata {
                file: "assistant.txt".to_string(),
                description: Some("General purpose assistant with tool usage instructions".to_string()),
                tags: vec![],
            }
        );

        prompts.insert(
            "code-reviewer".to_string(),
            PromptMetadata {
                file: "code-reviewer.txt".to_string(),
                description: Some("Code review focused prompt".to_string()),
                tags: vec!["coding".to_string(), "review".to_string()],
            }
        );

        prompts.insert(
            "rust-expert".to_string(),
            PromptMetadata {
                file: "rust-expert.txt".to_string(),
                description: Some("Rust programming expert prompt".to_string()),
                tags: vec!["rust".to_string(), "programming".to_string()],
            }
        );

        Self {
            prompts,
            default_prompt: Some("assistant".to_string()),
        }
    }
}

const DEFAULT_ASSISTANT_PROMPT: &str = r#"You are a helpful AI assistant with access to tools for file operations and bash commands.

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

const DEFAULT_CODE_REVIEWER_PROMPT: &str = "You are an expert code reviewer. Focus on code quality, security, performance, and best practices. Provide constructive feedback.";

const DEFAULT_RUST_EXPERT_PROMPT: &str = "You are a Rust programming expert. Help with Rust code, best practices, memory safety, and performance optimization.";

impl SystemPromptManager {
    pub fn new() -> Result<Self> {
        let config_path = Self::config_path()?;
        let prompts_dir = Self::prompts_dir()?;

        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            let default_config = SystemPromptsConfig::default();
            Self::initialize_default_prompts(&prompts_dir)?;
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

    fn prompts_dir() -> Result<PathBuf> {
        let prompts_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("hoosh")
            .join("prompts");

        fs::create_dir_all(&prompts_dir)
            .context("Failed to create prompts directory")?;

        Ok(prompts_dir)
    }

    fn initialize_default_prompts(prompts_dir: &PathBuf) -> Result<()> {
        let assistant_path = prompts_dir.join("assistant.txt");
        if !assistant_path.exists() {
            fs::write(&assistant_path, DEFAULT_ASSISTANT_PROMPT)
                .context("Failed to write default assistant prompt")?;
        }

        let code_reviewer_path = prompts_dir.join("code-reviewer.txt");
        if !code_reviewer_path.exists() {
            fs::write(&code_reviewer_path, DEFAULT_CODE_REVIEWER_PROMPT)
                .context("Failed to write default code reviewer prompt")?;
        }

        let rust_expert_path = prompts_dir.join("rust-expert.txt");
        if !rust_expert_path.exists() {
            fs::write(&rust_expert_path, DEFAULT_RUST_EXPERT_PROMPT)
                .context("Failed to write default rust expert prompt")?;
        }

        Ok(())
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

    fn load_prompt_content(&self, metadata: &PromptMetadata) -> Result<String> {
        let prompts_dir = Self::prompts_dir()?;
        let prompt_path = prompts_dir.join(&metadata.file);
        fs::read_to_string(&prompt_path)
            .with_context(|| format!("Failed to read prompt file: {}", metadata.file))
    }

    pub fn get_prompt(&self, name: &str) -> Option<SystemPrompt> {
        self.config.prompts.get(name).and_then(|metadata| {
            self.load_prompt_content(metadata).ok().map(|content| {
                SystemPrompt::from_metadata(name.to_string(), metadata.clone(), content)
            })
        })
    }

    pub fn get_default_prompt(&self) -> Option<SystemPrompt> {
        self.config.default_prompt.as_ref()
            .and_then(|name| self.get_prompt(name))
    }

    pub fn list_prompts(&self) -> Vec<SystemPrompt> {
        self.config.prompts.iter()
            .filter_map(|(name, metadata)| {
                self.load_prompt_content(metadata).ok().map(|content| {
                    SystemPrompt::from_metadata(name.clone(), metadata.clone(), content)
                })
            })
            .collect()
    }

    pub fn add_prompt(&mut self, name: String, content: String, description: Option<String>, tags: Vec<String>) -> Result<()> {
        let prompts_dir = Self::prompts_dir()?;
        let filename = format!("{}.txt", name);
        let prompt_path = prompts_dir.join(&filename);

        fs::write(&prompt_path, content)
            .context("Failed to write prompt file")?;

        let metadata = PromptMetadata {
            file: filename,
            description,
            tags,
        };

        self.config.prompts.insert(name, metadata);
        self.save()
    }

    pub fn remove_prompt(&mut self, name: &str) -> Result<bool> {
        if let Some(metadata) = self.config.prompts.remove(name) {
            let prompts_dir = Self::prompts_dir()?;
            let prompt_path = prompts_dir.join(&metadata.file);
            if prompt_path.exists() {
                fs::remove_file(&prompt_path)
                    .context("Failed to remove prompt file")?;
            }

            if self.config.default_prompt.as_ref() == Some(&name.to_string()) {
                self.config.default_prompt = None;
            }
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
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

    pub fn find_prompts_by_tag(&self, tag: &str) -> Vec<SystemPrompt> {
        self.config.prompts.iter()
            .filter_map(|(name, metadata)| {
                if metadata.tags.contains(&tag.to_string()) {
                    self.load_prompt_content(metadata).ok().map(|content| {
                        SystemPrompt::from_metadata(name.clone(), metadata.clone(), content)
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }
}