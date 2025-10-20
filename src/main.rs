use anyhow::Result;
use clap::Parser;
#[cfg(feature = "anthropic")]
use hoosh::backends::{AnthropicBackend, AnthropicConfig};
#[cfg(feature = "openai-compatible")]
use hoosh::backends::{OpenAICompatibleBackend, OpenAICompatibleConfig};
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
    let config = match AppConfig::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Warning: Failed to load config: {}. Using default config.",
                e
            );
            AppConfig::default()
        }
    };

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
    let backend_name = backend_name.unwrap_or_else(|| config.default_backend.clone());

    let backend: Box<dyn LlmBackend> = create_backend(&backend_name, config)?;

    let working_dir = if !add_dirs.is_empty() {
        PathBuf::from(&add_dirs[0])
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let parser = MessageParser::with_working_directory(working_dir.clone());
    let permission_manager = PermissionManager::new().with_skip_permissions(skip_permissions);

    let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(working_dir.clone());

    hoosh::tui::run(backend, parser, permission_manager, tool_registry, config.clone()).await?;

    Ok(())
}
fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    match backend_name {
        "mock" => {
            let _ = config; // Suppress unused warning when features are disabled
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
        #[cfg(feature = "anthropic")]
        "anthropic" => {
            let backend_config = config.get_backend_config("anthropic");
            let api_key = backend_config
                .and_then(|c| c.api_key.clone())
                .unwrap_or_default();
            let model = backend_config
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "claude-sonnet-4.5".to_string());
            let base_url = backend_config
                .and_then(|c| c.base_url.clone())
                .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());

            let anthropic_config = AnthropicConfig {
                api_key,
                model,
                base_url,
            };

            Ok(Box::new(AnthropicBackend::new(anthropic_config)?))
        }
        #[cfg(feature = "openai-compatible")]
        // Truly OpenAI-compatible providers (use max_completion_tokens): openai, groq, ollama
        name if matches!(name, "openai" | "ollama" | "groq") => {
            let backend_config = config.get_backend_config(name);

            // Get provider-specific defaults
            let (default_model, default_base_url) = match name {
                "openai" => ("gpt-4", "https://api.openai.com/v1"),
                "ollama" => ("llama3", "http://localhost:11434/v1"),
                "groq" => ("mixtral-8x7b-32768", "https://api.groq.com/openai/v1"),
                _ => ("", ""),
            };

            let api_key = backend_config
                .and_then(|c| c.api_key.clone())
                .unwrap_or_else(|| {
                    if name == "ollama" {
                        "ollama".to_string()
                    } else {
                        String::new()
                    }
                });
            let model = backend_config
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| default_model.to_string());
            let base_url = backend_config
                .and_then(|c| c.base_url.clone())
                .unwrap_or_else(|| default_base_url.to_string());
            let temperature = backend_config.and_then(|c| c.temperature);

            let openai_config = OpenAICompatibleConfig {
                name: name.to_string(),
                api_key,
                model,
                base_url,
                temperature,
            };

            Ok(Box::new(OpenAICompatibleBackend::new(openai_config)?))
        }
        _ => {
            let mut available = vec!["mock"];
            #[cfg(feature = "together-ai")]
            available.push("together_ai");
            #[cfg(feature = "openai-compatible")]
            available.extend_from_slice(&["openai", "ollama", "groq"]);
            #[cfg(feature = "anthropic")]
            available.push("anthropic");

            anyhow::bail!(
                "Unknown backend: {}. Available backends: {}",
                backend_name,
                available.join(", ")
            );
        }
    }
}

fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = AppConfig::load()?;
            console().plain(&format!("default_backend = \"{}\"", config.default_backend));
            if let Some(ref default_agent) = config.default_agent {
                console().plain(&format!("default_agent = \"{}\"", default_agent));
            }
            if let Some(ref verbosity) = config.verbosity {
                console().plain(&format!("verbosity = \"{}\"", verbosity));
            }
            console().plain(&format!("review_mode = {}", config.review_mode));

            if !config.agents.is_empty() {
                console().newline();
                console().plain("[agents]");
                for (agent_name, agent_config) in &config.agents {
                    console().plain(&format!(
                        "{} = {{ file = \"{}\"",
                        agent_name, agent_config.file
                    ));
                    if let Some(ref description) = agent_config.description {
                        console().plain(&format!("  description = \"{}\"", description));
                    }
                    if !agent_config.tags.is_empty() {
                        console().plain(&format!(
                            "  tags = [{}]",
                            agent_config
                                .tags
                                .iter()
                                .map(|t| format!("\"{}\"", t))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    console().plain("}");
                }
            }

            if !config.backends.is_empty() {
                console().newline();
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
                    if let Some(temperature) = backend_config.temperature {
                        console().plain(&format!("temperature = {}", temperature));
                    }
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
                        console().error(
                            "Invalid verbosity level. Valid options: quiet, normal, verbose, debug",
                        );
                        return Ok(());
                    }
                }
            } else if key == "default_agent" {
                config.default_agent = Some(value);
                config.save()?;
                console().success("Default agent configuration updated successfully");
            } else {
                // Handle backend config keys: <backend>_api_key, <backend>_model, <backend>_base_url, <backend>_temperature
                // Try to match known patterns
                let (backend_name, actual_key) = if key.ends_with("_api_key") {
                    (&key[..key.len() - 8], "api_key")
                } else if key.ends_with("_base_url") {
                    (&key[..key.len() - 9], "base_url")
                } else if key.ends_with("_model") {
                    (&key[..key.len() - 6], "model")
                } else if key.ends_with("_temperature") {
                    (&key[..key.len() - 12], "temperature")
                } else {
                    ("", "")
                };

                if !backend_name.is_empty() && !actual_key.is_empty() {
                    config.update_backend_setting(backend_name, actual_key, value)?;
                    config.save()?;
                    console().success("Backend configuration updated successfully");
                } else {
                    console().error(&format!(
                        "Unknown config key: {}. Use format: <backend>_<setting> where backend is one of [openai, together_ai, ollama, groq, anthropic] and setting is one of [api_key, model, base_url, temperature]",
                        key
                    ));
                }
            }
        }
    }
    Ok(())
}
