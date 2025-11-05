use crate::cli::ConfigAction;
use crate::{AppConfig, console};

pub fn handle_config(action: ConfigAction) -> anyhow::Result<()> {
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
