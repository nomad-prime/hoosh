use crate::commands::custom::CustomCommandManager;
use crate::config::AppConfig;
use crate::console::console;
use anyhow::{Context, Result};
use std::io::{self, Write};

pub fn handle_commands(action: super::CommandAction) -> Result<()> {
    match action {
        super::CommandAction::ReinstallBuiltins => reinstall_builtins(),
    }
}

fn reinstall_builtins() -> Result<()> {
    let commands_dir =
        AppConfig::commands_dir().context("Failed to resolve global commands directory")?;

    console().warning(&format!(
        "This will overwrite all built-in custom command files in {}.",
        commands_dir.display()
    ));
    console().info("Do you want to continue? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    if input != "y" && input != "yes" {
        console().info("Aborted.");
        return Ok(());
    }

    CustomCommandManager::install_default_commands_to(&commands_dir, true)?;
    let count = crate::config::DEFAULT_CUSTOM_COMMANDS.len();

    console().success(&format!(
        "Reinstalled {} built-in command file{}.",
        count,
        if count == 1 { "" } else { "s" }
    ));

    Ok(())
}
