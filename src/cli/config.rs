use crate::cli::ConfigAction;
use crate::{AppConfig, console};

enum ConfigKey {
    DefaultBackend,
    Verbosity,
    DefaultAgent,
    BackendSetting { backend: String, key: String },
}

impl ConfigKey {
    fn parse(key: &str) -> Result<Self, String> {
        match key {
            "default_backend" => Ok(Self::DefaultBackend),
            "verbosity" => Ok(Self::Verbosity),
            "default_agent" => Ok(Self::DefaultAgent),
            _ => Self::parse_backend_key(key),
        }
    }

    fn parse_backend_key(key: &str) -> Result<Self, String> {
        const SUFFIXES: &[(&str, &str)] = &[
            ("_api_key", "api_key"),
            ("_base_url", "base_url"),
            ("_chat_api", "chat_api"),
            ("_temperature", "temperature"),
            ("_model", "model"),
        ];

        for (suffix, setting_key) in SUFFIXES {
            if let Some(backend) = key.strip_suffix(suffix) {
                if !backend.is_empty() {
                    return Ok(Self::BackendSetting {
                        backend: backend.to_string(),
                        key: setting_key.to_string(),
                    });
                }
            }
        }

        Err(format!(
            "Unknown config key: {}. Use format: <backend>_<setting> where backend is one of \
             [openai, together_ai, ollama, anthropic] and setting is one of \
             [api_key, model, base_url, temperature, chat_api]",
            key
        ))
    }
}

fn mask_api_key(api_key: &str) -> String {
    let char_count = api_key.chars().count();
    if char_count > 8 {
        let chars: Vec<char> = api_key.chars().collect();
        let prefix: String = chars.iter().take(4).collect();
        let suffix: String = chars.iter().rev().take(4).rev().collect();
        format!("{}...{}", prefix, suffix)
    } else {
        "***".to_string()
    }
}

fn create_masked_config(config: &AppConfig) -> AppConfig {
    let mut masked_config = config.clone();

    for backend_config in masked_config.backends.values_mut() {
        if let Some(ref api_key) = backend_config.api_key {
            backend_config.api_key = Some(mask_api_key(api_key));
        }
    }

    masked_config
}

pub fn handle_config(action: ConfigAction) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show => {
            let config = AppConfig::load()?;
            let masked_config = create_masked_config(&config);

            let toml_output = toml::to_string_pretty(&masked_config)?;
            console().plain(&toml_output);
        }
        ConfigAction::Set { key, value } => {
            let mut config = AppConfig::load()?;
            let config_key = ConfigKey::parse(&key).map_err(|e| anyhow::anyhow!(e))?;

            match config_key {
                ConfigKey::DefaultBackend => {
                    config.default_backend = value;
                    config.save()?;
                    console().success("Configuration updated successfully");
                }
                ConfigKey::Verbosity => {
                    validate_verbosity(&value)?;
                    config.verbosity = Some(value);
                    config.save()?;
                    console().success("Verbosity configuration updated successfully");
                }
                ConfigKey::DefaultAgent => {
                    config.default_agent = Some(value);
                    config.save()?;
                    console().success("Default agent configuration updated successfully");
                }
                ConfigKey::BackendSetting { backend, key } => {
                    config.update_backend_setting(&backend, &key, value)?;
                    config.save()?;
                    console().success("Backend configuration updated successfully");
                }
            }
        }
    }
    Ok(())
}

fn validate_verbosity(value: &str) -> anyhow::Result<()> {
    match value {
        "quiet" | "normal" | "verbose" | "debug" => Ok(()),
        _ => Err(anyhow::anyhow!(
            "Invalid verbosity level. Valid options: quiet, normal, verbose, debug"
        )),
    }
}
