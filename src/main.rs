use anyhow::Result;
use clap::Parser;
use hoosh::cli::{handle_agent, handle_config, handle_conversations, handle_setup};
use hoosh::{
    cli::{Cli, Commands},
    config::{AppConfig, ConfigError},
    console::init_console,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { action }) => {
            handle_config(action)?;
        }
        Some(Commands::Conversations { action }) => {
            handle_conversations(action)?;
        }
        Some(Commands::Setup) => {
            // Don't load config before setup - let setup wizard handle it
            handle_setup().await?;
        }
        None => {
            // Try to load config; if it doesn't exist, run setup wizard first
            let config = match AppConfig::load() {
                Ok(config) => config,
                Err(e) => {
                    // Check if error is specifically about missing config file
                    if matches!(e, ConfigError::NotFound { .. }) {
                        eprintln!("No configuration found. Starting setup wizard...\n");
                        match handle_setup().await {
                            Ok(()) => {
                                // After setup, try to load the newly created config
                                match AppConfig::load() {
                                    Ok(cfg) => cfg,
                                    Err(load_err) => {
                                        eprintln!(
                                            "✗ Critical: Setup completed but config could not be loaded: {}",
                                            load_err
                                        );
                                        eprintln!("Please check your config file and try again.");
                                        return Err(load_err.into());
                                    }
                                }
                            }
                            Err(setup_err) => {
                                eprintln!("✗ Setup failed: {}", setup_err);
                                return Err(setup_err);
                            }
                        }
                    } else {
                        eprintln!(
                            "Warning: Failed to load config: {}. Using default config.",
                            e
                        );
                        AppConfig::default()
                    }
                }
            };

            // Initialize console with effective verbosity (CLI takes precedence over config)
            let effective_verbosity = cli.get_effective_verbosity(config.get_verbosity());
            init_console(effective_verbosity);

            handle_agent(
                cli.backend,
                cli.add_dir,
                cli.skip_permissions,
                cli.continue_last,
                &config,
            )
            .await?;
        }
    }

    Ok(())
}
