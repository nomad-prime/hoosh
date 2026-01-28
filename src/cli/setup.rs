use crate::cli::shell_setup::{
    ShellType, detect_shell, install_shell_alias as install_shell_function,
};
use crate::config::AppConfig;
use crate::console::console;
use crate::tui::setup::{run, save_wizard_result};
use crate::tui::terminal::{init_terminal, restore_terminal};
use anyhow::Result;

pub async fn handle_setup() -> Result<()> {
    let config_path = AppConfig::config_path()?;

    if config_path.exists() {
        console().info(&format!(
            "Configuration already exists at: {}",
            config_path.display()
        ));
        console().info("Do you want to reconfigure? (y/n)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            console().info("Setup cancelled.");
            return Ok(());
        }
    }

    let terminal = init_terminal()?;

    let (terminal, result) = run(terminal).await?;

    restore_terminal(terminal)?;

    if let Some(result) = result {
        match save_wizard_result(&result) {
            Ok(()) => {
                console().info("\n✓ Configuration saved");
                console().info(&format!("  Backend: {}", result.backend));
                console().info(&format!("  Model: {}", result.model));

                if result.api_key.is_some() {
                    console().info("  API Key: Set");
                } else {
                    console().info("  API Key: Not set");
                }

                match AppConfig::load() {
                    Ok(_) => {}
                    Err(e) => {
                        console().error(&format!(
                            "\n✗ Configuration saved but could not be loaded: {}",
                            e
                        ));
                        console().error(&format!(
                            "Check your config file at: {}",
                            AppConfig::config_path()?.display()
                        ));
                        return Err(e.into());
                    }
                }

                prompt_shell_setup()?;
            }
            Err(e) => {
                console().error(&format!("\n✗ Failed to save configuration: {}", e));
                console().error("Check:");
                console().error("  1. Write permissions to ~/.config/hoosh/");
                console().error("  2. Sufficient disk space");
                return Err(e);
            }
        }
    } else {
        console().info("Setup cancelled.");
    }

    Ok(())
}

fn prompt_shell_setup() -> Result<()> {
    console().info("\n─── Shell Integration ───");
    console().info("Would you like to set up the @hoosh shell alias?");
    console().info("This enables quick invocations like: @hoosh \"what files changed?\"");
    console().info("Install shell integration? (y/n)");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        console().info("Shell integration skipped.");
        console().info("You can run 'hoosh setup' again later to install it.");
        return Ok(());
    }

    match detect_shell() {
        Ok(shell_type) => {
            console().info(&format!("Detected shell: {}", shell_type_name(&shell_type)));
            install_shell_function(shell_type)?;
        }
        Err(e) => {
            console().warning(&format!("Could not detect shell: {}", e));
            console().info("Shell integration skipped.");
        }
    }

    Ok(())
}

fn shell_type_name(shell_type: &ShellType) -> &'static str {
    match shell_type {
        ShellType::Bash => "Bash",
        ShellType::Zsh => "Zsh",
        ShellType::Fish => "Fish",
        #[cfg(windows)]
        ShellType::PowerShell => "PowerShell",
    }
}
