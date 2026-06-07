use crate::commands::CommandRegistry;
use crate::commands::custom::parser::{ParsedCommand, parse_command_file};
use crate::commands::custom::wrapper::CustomCommandWrapper;
use crate::config::AppConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CustomCommandManager {
    global_dir: PathBuf,
    project_dir: PathBuf,
    loaded_commands: Vec<ParsedCommand>,
}

impl CustomCommandManager {
    pub fn new() -> Result<Self> {
        let global_dir =
            AppConfig::commands_dir().context("Failed to resolve global commands directory")?;
        let project_dir = Self::project_commands_dir()?;

        Self::install_default_commands(&global_dir)?;

        Ok(Self {
            global_dir,
            project_dir,
            loaded_commands: Vec::new(),
        })
    }

    /// Drop hoosh's default custom commands into `dir`, but never overwrite a
    /// file the user has already created or edited.
    fn install_default_commands(dir: &std::path::Path) -> Result<()> {
        for (file_name, content) in crate::config::DEFAULT_CUSTOM_COMMANDS {
            let path = dir.join(file_name);
            if path.exists() {
                continue;
            }
            fs::write(&path, content).with_context(|| {
                format!("Failed to install default command: {}", path.display())
            })?;
        }
        Ok(())
    }

    fn project_commands_dir() -> Result<PathBuf> {
        let current_dir = env::current_dir().context("Could not determine current directory")?;
        Ok(current_dir.join(".hoosh").join("commands"))
    }

    /// Load commands from the global dir first, then layer project commands on
    /// top. A project file with the same command name overrides the global one.
    pub fn load_commands(&mut self) -> Result<()> {
        let mut by_name: HashMap<String, ParsedCommand> = HashMap::new();

        for dir in [&self.global_dir, &self.project_dir] {
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }

                match parse_command_file(&path) {
                    Ok(command) => {
                        by_name.insert(command.name.clone(), command);
                    }
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

        self.loaded_commands = by_name.into_values().collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_cmd(dir: &std::path::Path, name: &str, description: &str) {
        let body = format!(
            "---\ndescription: {description}\n---\nbody for {name}\n",
            description = description,
            name = name,
        );
        fs::write(dir.join(format!("{}.md", name)), body).unwrap();
    }

    fn load_with_dirs(global: &std::path::Path, project: &std::path::Path) -> Vec<ParsedCommand> {
        let mut m = CustomCommandManager {
            global_dir: global.to_path_buf(),
            project_dir: project.to_path_buf(),
            loaded_commands: Vec::new(),
        };
        m.load_commands().unwrap();
        m.loaded_commands
    }

    #[test]
    fn loads_global_commands() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();
        write_cmd(global.path(), "alpha", "from global");

        let cmds = load_with_dirs(global.path(), project.path());
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "alpha");
        assert_eq!(
            Some(cmds[0].metadata.description.as_str()),
            Some("from global")
        );
    }

    #[test]
    fn project_command_overrides_global_by_name() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();
        write_cmd(global.path(), "alpha", "from global");
        write_cmd(project.path(), "alpha", "from project");

        let cmds = load_with_dirs(global.path(), project.path());
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            Some(cmds[0].metadata.description.as_str()),
            Some("from project")
        );
    }

    #[test]
    fn global_and_project_commands_are_unioned() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();
        write_cmd(global.path(), "alpha", "from global");
        write_cmd(project.path(), "beta", "from project");

        let cmds = load_with_dirs(global.path(), project.path());
        let mut names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn missing_project_dir_is_ok() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();
        let missing = project.path().join("does-not-exist");
        write_cmd(global.path(), "alpha", "from global");

        let cmds = load_with_dirs(global.path(), &missing);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "alpha");
    }
}
