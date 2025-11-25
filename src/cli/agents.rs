use crate::agent_definition::AgentDefinitionManager;
use crate::config::AppConfig;
use anyhow::Result;
use std::io::{self, Write};

pub fn handle_agents(action: super::AgentAction) -> Result<()> {
    match action {
        super::AgentAction::ReinstallBuiltins => reinstall_builtins(),
    }
}

fn reinstall_builtins() -> Result<()> {
    println!("This will overwrite all built-in agent files (hoosh_*.txt).");
    print!("Do you want to continue? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    if input != "y" && input != "yes" {
        println!("Aborted.");
        return Ok(());
    }

    let agents_dir = AppConfig::agents_dir()?;
    AgentDefinitionManager::initialize_default_agents(&agents_dir)?;

    println!("✓ Built-in agent files have been reinstalled.");

    Ok(())
}
