use crate::config::AppConfig;
use crate::tui::setup::{run, save_wizard_result};
use crate::tui::terminal::{init_terminal, restore_terminal};
use anyhow::Result;

pub async fn handle_setup() -> Result<()> {
    let config_path = AppConfig::config_path()?;

    if config_path.exists() {
        println!("Configuration already exists at: {}", config_path.display());
        println!("Do you want to reconfigure? (y/n)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Setup cancelled.");
            return Ok(());
        }
    }

    let terminal = init_terminal()?;

    let (terminal, result) = run(terminal).await?;

    restore_terminal(terminal)?;

    if let Some(result) = result {
        match save_wizard_result(&result) {
            Ok(()) => {
                println!("\n✓ Configuration saved");
                println!("  Backend: {}", result.backend);
                println!("  Model: {}", result.model);

                if result.api_key.is_some() {
                    println!("  API Key: Set");
                } else {
                    println!("  API Key: Not set");
                }

                // Verify the config can be loaded
                match AppConfig::load() {
                    Ok(_) => {
                        println!()
                    }
                    Err(e) => {
                        eprintln!("\n✗ Configuration saved but could not be loaded: {}", e);
                        eprintln!(
                            "Check your config file at: {}",
                            AppConfig::config_path()?.display()
                        );
                        return Err(e.into());
                    }
                }
            }
            Err(e) => {
                eprintln!("\n✗ Failed to save configuration: {}", e);
                eprintln!("Check:");
                eprintln!("  1. Write permissions to ~/.config/hoosh/");
                eprintln!("  2. Sufficient disk space");
                return Err(e);
            }
        }
    } else {
        println!("Setup cancelled.");
    }

    Ok(())
}
