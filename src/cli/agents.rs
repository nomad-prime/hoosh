use crate::agent_definition::AgentDefinitionManager;
use crate::config::{AgentConfig, AppConfig};
use crate::console::console;
use anyhow::Result;
use std::fs;
use std::io::{self, Write};

pub fn handle_agents(action: super::AgentAction) -> Result<()> {
    match action {
        super::AgentAction::ReinstallBuiltins => reinstall_builtins(),
        super::AgentAction::Create { name, description } => create_custom_agent(&name, description),
    }
}

fn create_custom_agent(name: &str, description: Option<String>) -> Result<()> {
    if name.starts_with("hoosh_") {
        return Err(anyhow::anyhow!(
            "Custom agent names cannot start with 'hoosh_' (reserved for built-in agents)"
        ));
    }

    let agents_dir = AppConfig::agents_dir()?;
    let filename = format!("{}.txt", name);
    let agent_path = agents_dir.join(&filename);

    if agent_path.exists() {
        return Err(anyhow::anyhow!(
            "Agent file already exists: {}",
            agent_path.display()
        ));
    }

    let template = "You are a helpful AI assistant.\n\n[Add your custom instructions here]\n";
    fs::write(&agent_path, template)?;

    let mut config = AppConfig::load()?;
    config.agents.insert(
        name.to_string(),
        AgentConfig {
            file: filename,
            description,
            tags: vec![],
            core_instructions_file: None,
        },
    );
    config.save()?;

    console().info(&format!("Created custom agent: {}", name));
    console().info(&format!("Edit: {}", agent_path.display()));

    Ok(())
}

fn reinstall_builtins() -> Result<()> {
    console().warning("This will overwrite all built-in agent files (hoosh_*.txt).");
    console().info("Do you want to continue? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    if input != "y" && input != "yes" {
        console().info("Aborted.");
        return Ok(());
    }

    let agents_dir = AppConfig::agents_dir()?;
    AgentDefinitionManager::initialize_default_agents(&agents_dir)?;

    console().info("Built-in agent files have been reinstalled.");

    Ok(())
}
