use crate::config::AppConfig;
use crate::console::console;
use crate::tui::setup::{run, save_wizard_result};
use crate::tui::terminal::{init_terminal, restore_terminal};
use anyhow::Result;

pub async fn handle_setup() -> Result<()> {
    let config_path = AppConfig::config_path()?;

    if config_path.exists() {
        console().info(&format!("Configuration already exists at: {}", config_path.display()));
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
                        console().error(&format!("\n✗ Configuration saved but could not be loaded: {}", e));
                        console().error(&format!(
                            "Check your config file at: {}",
                            AppConfig::config_path()?.display()
                        ));
                        return Err(e.into());
                    }
                }
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
