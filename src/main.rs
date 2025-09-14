use clap::Parser;
use hoosh::{cli::{Cli, Commands, ConfigAction}, config::AppConfig, backends::{LlmBackend, MockBackend}};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Chat { backend, message } => {
            handle_chat(backend, message).await?;
        }
        Commands::Config { action } => {
            handle_config(action)?;
        }
    }

    Ok(())
}

async fn handle_chat(backend_name: Option<String>, message: Option<String>) -> Result<()> {
    let config = AppConfig::load()?;
    let backend_name = backend_name.unwrap_or(config.default_backend);

    let backend: Box<dyn LlmBackend> = match backend_name.as_str() {
        "mock" => Box::new(MockBackend::new()),
        _ => {
            eprintln!("Unknown backend: {}", backend_name);
            return Ok(());
        }
    };

    if let Some(msg) = message {
        let response = backend.send_message(&msg).await?;
        println!("{}", response);
    } else {
        println!("Interactive chat mode not implemented yet. Use --message to send a single message.");
    }

    Ok(())
}

fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = AppConfig::load()?;
            println!("default_backend = \"{}\"", config.default_backend);
        }
        ConfigAction::Set { key, value } => {
            let mut config = AppConfig::load()?;
            match key.as_str() {
                "default_backend" => {
                    config.default_backend = value;
                    config.save()?;
                    println!("Configuration updated successfully");
                }
                _ => {
                    eprintln!("Unknown config key: {}", key);
                }
            }
        }
    }
    Ok(())
}
