use anyhow::Result;
use clap::Parser;
#[cfg(feature = "together-ai")]
use hoosh::backends::{TogetherAiBackend, TogetherAiConfig};
use hoosh::{
    backends::{LlmBackend, MockBackend},
    cli::{Cli, Commands, ConfigAction},
    config::AppConfig,
    console::{console, init_console},
    parser::MessageParser,
    permissions::PermissionManager,
    tool_executor::ToolExecutor,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config to get configured verbosity level
    let config = AppConfig::load().unwrap_or_default();

    // Initialize console with effective verbosity (CLI takes precedence over config)
    let effective_verbosity = cli.get_effective_verbosity(config.get_verbosity());
    init_console(effective_verbosity);

    match cli.command {
        Some(Commands::Config { action }) => {
            handle_config(action)?;
        }
        None => {
            // Default to chat mode
            handle_chat(cli.backend, cli.add_dir, cli.skip_permissions, &config).await?;
        }
    }

    Ok(())
}

async fn handle_chat(
    backend_name: Option<String>,
    add_dirs: Vec<String>,
    skip_permissions: bool,
    config: &AppConfig,
) -> Result<()> {
    let backend_name = backend_name.unwrap_or(config.default_backend.clone());

    let backend: Box<dyn LlmBackend> = create_backend(&backend_name, &config)?;

    let working_dir = if !add_dirs.is_empty() {
        PathBuf::from(&add_dirs[0])
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let parser = MessageParser::with_working_directory(working_dir.clone());
    let permission_manager = PermissionManager::new().with_skip_permissions(skip_permissions);

    let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(working_dir.clone());

    hoosh::tui::run(backend, parser, permission_manager, tool_registry).await?;

    Ok(())
}
fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    match backend_name {
        "mock" => {
            let _ = config; // Suppress unused warning when together-ai feature is disabled
            Ok(Box::new(MockBackend::new()))
        }
        #[cfg(feature = "together-ai")]
        "together_ai" => {
            let backend_config = config.get_backend_config("together_ai");
            let api_key = backend_config
                .and_then(|c| c.api_key.clone())
                .unwrap_or_default();
            let model = backend_config
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "meta-llama/Llama-2-7b-chat-hf".to_string());
            let base_url = backend_config
                .and_then(|c| c.base_url.clone())
                .unwrap_or_else(|| "https://api.together.xyz/v1".to_string());

            let together_config = TogetherAiConfig {
                api_key,
                model,
                base_url,
            };

            Ok(Box::new(TogetherAiBackend::new(together_config)?))
        }
        _ => {
            #[cfg(feature = "together-ai")]
            let available = "mock, together_ai";
            #[cfg(not(feature = "together-ai"))]
            let available =
                "mock (together_ai requires Rust 1.82+ - enable with --features together-ai)";
            anyhow::bail!(
                "Unknown backend: {}. Available backends: {}",
                backend_name,
                available
            );
        }
    }
}

fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = AppConfig::load()?;
            console().plain(&format!("default_backend = \"{}\"", config.default_backend));
            if let Some(ref verbosity) = config.verbosity {
                console().plain(&format!("verbosity = \"{}\"", verbosity));
            }

            for (backend_name, backend_config) in &config.backends {
                console().newline();
                console().plain(&format!("[{}]", backend_name));
                if let Some(ref api_key) = backend_config.api_key {
                    // Use char-based slicing to safely handle UTF-8 and avoid panics
                    let masked_key = if api_key.chars().count() > 8 {
                        let chars: Vec<char> = api_key.chars().collect();
                        let prefix: String = chars.iter().take(4).collect();
                        let suffix: String = chars.iter().rev().take(4).rev().collect();
                        format!("{}...{}", prefix, suffix)
                    } else {
                        "***".to_string()
                    };
                    console().plain(&format!("api_key = \"{}\"", masked_key));
                }
                if let Some(ref model) = backend_config.model {
                    console().plain(&format!("model = \"{}\"", model));
                }
                if let Some(ref base_url) = backend_config.base_url {
                    console().plain(&format!("base_url = \"{}\"", base_url));
                }
            }
        }
        ConfigAction::Set { key, value } => {
            let mut config = AppConfig::load()?;

            if key == "default_backend" {
                config.default_backend = value;
                config.save()?;
                console().success("Configuration updated successfully");
            } else if key == "verbosity" {
                match value.as_str() {
                    "quiet" | "normal" | "verbose" | "debug" => {
                        config.verbosity = Some(value);
                        config.save()?;
                        console().success("Verbosity configuration updated successfully");
                    }
                    _ => {
                        console().error("Invalid verbosity level. Valid options: quiet, normal, verbose, debug");
                        return Ok(());
                    }
                }
            } else if let Some((backend_name, setting_key)) = key.split_once('_') {
                if matches!(backend_name, "together")
                    && matches!(setting_key, "ai_api_key" | "ai_model" | "ai_base_url")
                {
                    // Handle together_ai_* keys by splitting further
                    if setting_key.starts_with("ai_") {
                        let actual_key = &setting_key[3..]; // Remove "ai_" prefix
                        config.update_backend_setting("together_ai", actual_key, value)?;
                        config.save()?;
                        console().success("Backend configuration updated successfully");
                    } else {
                        console().error(&format!("Unknown config key: {}. Available keys: default_backend, verbosity, together_ai_api_key, together_ai_model, together_ai_base_url", key));
                    }
                } else {
                    console().error(&format!("Unknown config key: {}. Available keys: default_backend, verbosity, together_ai_api_key, together_ai_model, together_ai_base_url", key));
                }
            } else {
                console().error(&format!("Unknown config key: {}. Available keys: default_backend, verbosity, together_ai_api_key, together_ai_model, together_ai_base_url", key));
            }
        }
    }
    Ok(())
}

