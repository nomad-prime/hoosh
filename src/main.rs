use anyhow::Result;
use clap::Parser;
use hoosh::cli::{handle_agent, handle_config, handle_conversations};
use hoosh::{
    cli::{Cli, Commands},
    config::AppConfig,
    console::init_console,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config to get configured verbosity level
    let config = AppConfig::load().unwrap_or_else(|e| {
        eprintln!(
            "Warning: Failed to load config: {}. Using default config.",
            e
        );
        AppConfig::default()
    });

    // Initialize console with effective verbosity (CLI takes precedence over config)
    let effective_verbosity = cli.get_effective_verbosity(config.get_verbosity());
    init_console(effective_verbosity);

    match cli.command {
        Some(Commands::Config { action }) => {
            handle_config(action)?;
        }
        Some(Commands::Conversations { action }) => {
            handle_conversations(action)?;
        }
        None => {
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
