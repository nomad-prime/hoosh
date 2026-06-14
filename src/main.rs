use anyhow::Result;
use clap::Parser;
use hoosh::cli::{
    handle_agent, handle_agents, handle_alias_install, handle_commands, handle_config,
    handle_conversations, handle_daemon, handle_setup, handle_shell,
};
use hoosh::session_files::cleanup_stale_sessions;
use hoosh::{
    cli::{Cli, Commands},
    config::{AppConfig, ConfigError, set_config_path_override, set_data_dir_override},
    console::{VerbosityLevel, init_console},
    logging::init_logging,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Guard must live for the whole program — dropping it stops the async log
    // worker and we'd lose pending writes on exit.
    let _log_guard = match init_logging() {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Warning: failed to initialise logging: {e}");
            None
        }
    };
    tracing::info!("hoosh starting (version {})", env!("CARGO_PKG_VERSION"));

    // Cleanup stale session files on startup (>7 days old)
    // This runs silently in the background - failures are non-fatal
    let _ = cleanup_stale_sessions();

    let cli = Cli::parse();

    if let Some(config_path) = &cli.config {
        set_config_path_override(config_path.clone());
    }

    if let Some(data_dir) = &cli.data_dir {
        set_data_dir_override(data_dir.clone());
    }

    if matches!(
        cli.command,
        Some(Commands::Config { .. })
            | Some(Commands::Conversations { .. })
            | Some(Commands::Agent { .. })
            | Some(Commands::Command { .. })
            | Some(Commands::Alias { .. })
            | Some(Commands::Daemon { .. })
    ) {
        init_console(cli.get_effective_verbosity(VerbosityLevel::Normal));
    }

    match cli.command {
        Some(Commands::Config { action }) => {
            if let Err(e) = AppConfig::ensure_project_config() {
                eprintln!("Warning: Failed to create project config: {}", e);
            }
            handle_config(action)?;
        }
        Some(Commands::Conversations { action }) => {
            if let Err(e) = AppConfig::ensure_project_config() {
                eprintln!("Warning: Failed to create project config: {}", e);
            }
            let config = AppConfig::load().unwrap_or_default();
            handle_conversations(action, &config)?;
        }
        Some(Commands::Agent { action }) => {
            handle_agents(action)?;
        }
        Some(Commands::Command { action }) => {
            handle_commands(action)?;
        }
        Some(Commands::Alias { action }) => {
            use hoosh::cli::AliasAction;
            match action {
                AliasAction::Install => handle_alias_install()?,
            }
        }
        Some(Commands::Setup) => {
            handle_setup().await?;
        }
        Some(Commands::Shell) => {
            let config = AppConfig::load().unwrap_or_default();
            init_console(cli.get_effective_verbosity(config.get_verbosity()));
            handle_shell(cli.backend, cli.skip_permissions, &config).await?;
        }
        Some(Commands::Daemon { action }) => {
            let config = AppConfig::load().unwrap_or_default();
            handle_daemon(action, config).await?;
        }
        None => {
            let config = match AppConfig::load() {
                Ok(config) => config,
                Err(e) => {
                    if matches!(e, ConfigError::NotFound { .. }) {
                        eprintln!("No configuration found. Starting setup wizard...\n");
                        match handle_setup().await {
                            Ok(()) => match AppConfig::load() {
                                Ok(cfg) => cfg,
                                Err(load_err) => {
                                    eprintln!(
                                        "✗ Critical: Setup completed but config could not be loaded: {}",
                                        load_err
                                    );
                                    eprintln!("Please check your config file and try again.");
                                    return Err(load_err.into());
                                }
                            },
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

            if let Err(e) = AppConfig::ensure_project_config() {
                eprintln!("Warning: Failed to create project config: {}", e);
            }

            let effective_verbosity = cli.get_effective_verbosity(config.get_verbosity());
            init_console(effective_verbosity);

            handle_agent(
                cli.backend,
                cli.add_dir,
                cli.skip_permissions,
                cli.continue_last,
                cli.resume,
                cli.name,
                cli.no_session_persistence,
                cli.mode,
                cli.memory_mode,
                cli.output_format,
                cli.message,
                &config,
            )
            .await?;
        }
    }

    Ok(())
}
