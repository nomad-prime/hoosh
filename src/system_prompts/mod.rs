use anyhow::{Context, Result};
use crate::config::{AppConfig, PromptConfig};
use serde::{Deserialize, Serialize};
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

pub struct SystemPromptManager {
    config: AppConfig,
}

impl SystemPrompt {
    pub fn from_config(name: String, config: PromptConfig, content: String) -> Self {
        Self {
            name,
            content,
            file: config.file,
            description: config.description,
            tags: config.tags,
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

impl SystemPromptManager {
    pub fn new() -> Result<Self> {
        let config = AppConfig::load()?;
        let prompts_dir = Self::prompts_dir()?;
        Self::initialize_default_prompts(&prompts_dir)?;

        Ok(Self { config })
    }

    fn prompts_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Failed to get home directory")?;
        let prompts_dir = PathBuf::from(home)
            .join(".config")
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

        Ok(())
    }

    fn load_prompt_content(&self, prompt_config: &PromptConfig) -> Result<String> {
        let prompts_dir = Self::prompts_dir()?;
        let prompt_path = prompts_dir.join(&prompt_config.file);
        fs::read_to_string(&prompt_path)
            .with_context(|| format!("Failed to read prompt file: {}", prompt_config.file))
    }

    pub fn get_prompt(&self, name: &str) -> Option<SystemPrompt> {
        self.config.prompts.get(name).and_then(|prompt_config| {
            self.load_prompt_content(prompt_config).ok().map(|content| {
                SystemPrompt::from_config(name.to_string(), prompt_config.clone(), content)
            })
        })
    }

    pub fn get_default_prompt(&self) -> Option<SystemPrompt> {
        self.config.default_prompt.as_ref()
            .and_then(|name| self.get_prompt(name))
    }

    pub fn list_prompts(&self) -> Vec<SystemPrompt> {
        self.config.prompts.iter()
            .filter_map(|(name, prompt_config)| {
                self.load_prompt_content(prompt_config).ok().map(|content| {
                    SystemPrompt::from_config(name.clone(), prompt_config.clone(), content)
                })
            })
            .collect()
    }
}