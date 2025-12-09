use crate::commands::CommandRegistry;
use crate::commands::custom::parser::{ParsedCommand, parse_command_file};
use crate::commands::custom::wrapper::CustomCommandWrapper;
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CustomCommandManager {
    commands_dir: PathBuf,
    loaded_commands: Vec<ParsedCommand>,
}

impl CustomCommandManager {
    pub fn new() -> Result<Self> {
        let commands_dir = Self::commands_dir()?;

        if !commands_dir.exists() {
            fs::create_dir_all(&commands_dir).with_context(|| {
                format!(
                    "Failed to create commands directory: {}",
                    commands_dir.display()
                )
            })?;
            eprintln!(
                "Created custom commands directory: {}",
                commands_dir.display()
            );
        }

        Ok(Self {
            commands_dir,
            loaded_commands: Vec::new(),
        })
    }

    fn commands_dir() -> Result<PathBuf> {
        let current_dir = env::current_dir().context("Could not determine current directory")?;
        Ok(current_dir.join(".hoosh").join("commands"))
    }

    pub fn load_commands(&mut self) -> Result<()> {
        if !self.commands_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.commands_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match parse_command_file(&path) {
                    Ok(command) => self.loaded_commands.push(command),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to load command from {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn register_commands(&self, registry: &mut CommandRegistry) -> Result<usize> {
        let mut registered_count = 0;

        for command in &self.loaded_commands {
            let wrapper = Arc::new(CustomCommandWrapper::new(command.clone()));

            match registry.register(wrapper) {
                Ok(()) => registered_count += 1,
                Err(e) => {
                    eprintln!(
                        "Warning: Could not register custom command '{}': {}",
                        command.name, e
                    );
                }
            }
        }

        Ok(registered_count)
    }

    pub fn list_commands(&self) -> Vec<&ParsedCommand> {
        self.loaded_commands.iter().collect()
    }
}
