use crate::AppConfig;
#[cfg(feature = "anthropic")]
use crate::backends::{AnthropicBackend, AnthropicConfig};
use crate::backends::{BackendKind, LlmBackend, MockBackend, OllamaBackend, OllamaConfig};
#[cfg(feature = "openai-compatible")]
use crate::backends::{OpenAICompatibleBackend, OpenAICompatibleConfig};
#[cfg(feature = "together-ai")]
use crate::backends::{TogetherAiBackend, TogetherAiConfig};
use crate::config::BackendConfig;
use anyhow::Result;
use std::str::FromStr;

pub trait BackendFactory {
    fn create(config: &BackendConfig, name: &str) -> Result<Box<dyn LlmBackend>>;
}

#[cfg(feature = "together-ai")]
impl BackendFactory for TogetherAiBackend {
    fn create(config: &BackendConfig, _name: &str) -> Result<Box<dyn LlmBackend>> {
        let api_key = config.api_key.clone().unwrap_or_default();
        let model = config.model.clone().unwrap_or_else(|| {
            BackendKind::TogetherAi
                .default_model()
                .unwrap_or("")
                .to_string()
        });
        let base_url = config.base_url.clone().unwrap_or_else(|| {
            BackendKind::TogetherAi
                .default_base_url()
                .unwrap_or("")
                .to_string()
        });

        let together_config = TogetherAiConfig {
            api_key,
            model,
            base_url,
            streaming: config.streaming.unwrap_or(true),
        };

        Ok(Box::new(TogetherAiBackend::new(together_config)?))
    }
}

#[cfg(feature = "anthropic")]
impl BackendFactory for AnthropicBackend {
    fn create(config: &BackendConfig, _name: &str) -> Result<Box<dyn LlmBackend>> {
        let api_key = config.api_key.clone().unwrap_or_default();
        let model = config.model.clone().unwrap_or_else(|| {
            BackendKind::Anthropic
                .default_model()
                .unwrap_or("")
                .to_string()
        });
        let base_url = config.base_url.clone().unwrap_or_else(|| {
            BackendKind::Anthropic
                .default_base_url()
                .unwrap_or("")
                .to_string()
        });

        let anthropic_config = AnthropicConfig {
            api_key,
            model,
            base_url,
            thinking_budget: config.thinking_budget,
            streaming: config.streaming.unwrap_or(true),
        };

        Ok(Box::new(AnthropicBackend::new(anthropic_config)?))
    }
}

#[cfg(feature = "openai-compatible")]
impl BackendFactory for OpenAICompatibleBackend {
    fn create(config: &BackendConfig, name: &str) -> Result<Box<dyn LlmBackend>> {
        let kind = BackendKind::from_str(name).ok();
        let default = |f: fn(&BackendKind) -> Option<&'static str>| {
            kind.as_ref().and_then(f).unwrap_or("").to_string()
        };

        let api_key = config.api_key.clone().unwrap_or_default();
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| default(BackendKind::default_model));
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| default(BackendKind::default_base_url));

        let chat_api = config
            .chat_api
            .clone()
            .unwrap_or_else(|| default(BackendKind::default_chat_api));

        let openai_config = OpenAICompatibleConfig {
            name: name.to_string(),
            api_key,
            model,
            base_url,
            chat_api,
            temperature: config.temperature,
            pricing_endpoint: config.pricing_endpoint.clone(),
            thinking_budget: config.thinking_budget,
            reasoning_effort: config.reasoning_effort,
            streaming: config.streaming.unwrap_or(true),
        };

        Ok(Box::new(OpenAICompatibleBackend::new(openai_config)?))
    }
}

impl BackendFactory for OllamaBackend {
    fn create(config: &BackendConfig, name: &str) -> Result<Box<dyn LlmBackend>> {
        let model = config.model.clone().unwrap_or_else(|| {
            BackendKind::Ollama
                .default_model()
                .unwrap_or("")
                .to_string()
        });
        let base_url = config.base_url.clone().unwrap_or_else(|| {
            BackendKind::Ollama
                .default_base_url()
                .unwrap_or("")
                .to_string()
        });

        let ollama_config = OllamaConfig {
            name: name.to_string(),
            model,
            base_url,
            temperature: config.temperature,
            streaming: config.streaming.unwrap_or(true),
        };

        Ok(Box::new(OllamaBackend::new(ollama_config)?))
    }
}
pub fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    let _backend_config = config
        .get_backend_config(backend_name)
        .ok_or_else(|| anyhow::anyhow!("Backend '{}' not found in config", backend_name))?;

    let kind = BackendKind::from_str(backend_name)
        .ok()
        .filter(BackendKind::is_available)
        .ok_or_else(|| unknown_backend_error(backend_name))?;

    #[allow(unreachable_patterns)]
    match kind {
        BackendKind::Mock => Ok(Box::new(MockBackend::new())),
        #[cfg(feature = "together-ai")]
        BackendKind::TogetherAi => TogetherAiBackend::create(_backend_config, backend_name),
        #[cfg(feature = "anthropic")]
        BackendKind::Anthropic => AnthropicBackend::create(_backend_config, backend_name),
        #[cfg(feature = "ollama")]
        BackendKind::Ollama => OllamaBackend::create(_backend_config, backend_name),
        #[cfg(feature = "openai-compatible")]
        BackendKind::OpenAi | BackendKind::Groq => {
            OpenAICompatibleBackend::create(_backend_config, backend_name)
        }
        _ => Err(unknown_backend_error(backend_name)),
    }
}

fn unknown_backend_error(backend_name: &str) -> anyhow::Error {
    let available: Vec<&str> = BackendKind::available()
        .iter()
        .map(BackendKind::as_str)
        .collect();
    anyhow::anyhow!(
        "Unknown backend: {}. Available backends: {}",
        backend_name,
        available.join(", ")
    )
}
