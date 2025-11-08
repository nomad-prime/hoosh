use crate::config::AppConfig;
use crate::tui::setup_wizard_loop;
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

    let (terminal, result) = setup_wizard_loop::run(terminal).await?;

    restore_terminal(terminal)?;

    if let Some(result) = result {
        setup_wizard_loop::save_wizard_result(&result)?;

        println!("\n✓ Configuration saved successfully!");
        println!("  Backend: {}", result.backend);
        println!("  Model: {}", result.model);

        if result.api_key.is_some() {
            println!("  API Key: Stored in config file");
        } else {
            let env_var_name = format!(
                "{}_API_KEY",
                result.backend.to_uppercase().replace("-", "_")
            );
            println!("  API Key: Using environment variable {}", env_var_name);
        }

        println!("\nYou can now run 'hoosh' to start chatting!");
    } else {
        println!("Setup cancelled.");
    }

    Ok(())
}
