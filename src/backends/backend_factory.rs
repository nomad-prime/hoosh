use crate::AppConfig;
use crate::backends::{
    AnthropicBackend, AnthropicConfig, LlmBackend, MockBackend, OllamaBackend, OllamaConfig,
    OpenAICompatibleBackend, OpenAICompatibleConfig, TogetherAiBackend, TogetherAiConfig,
};
use crate::config::BackendConfig;
use anyhow::Result;

pub trait BackendFactory {
    fn create(config: &BackendConfig, name: &str) -> Result<Box<dyn LlmBackend>>;
}

impl BackendFactory for TogetherAiBackend {
    fn create(config: &BackendConfig, _name: &str) -> Result<Box<dyn LlmBackend>> {
        let api_key = config.api_key.clone().unwrap_or_default();
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "meta-llama/Llama-2-7b-chat-hf".to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.together.xyz/v1".to_string());

        let together_config = TogetherAiConfig {
            api_key,
            model,
            base_url,
        };

        Ok(Box::new(TogetherAiBackend::new(together_config)?))
    }
}

impl BackendFactory for AnthropicBackend {
    fn create(config: &BackendConfig, _name: &str) -> Result<Box<dyn LlmBackend>> {
        let api_key = config.api_key.clone().unwrap_or_default();
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4.5".to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());

        let anthropic_config = AnthropicConfig {
            api_key,
            model,
            base_url,
        };

        Ok(Box::new(AnthropicBackend::new(anthropic_config)?))
    }
}

impl BackendFactory for OpenAICompatibleBackend {
    fn create(config: &BackendConfig, name: &str) -> Result<Box<dyn LlmBackend>> {
        let (default_model, default_base_url, default_chat_api) = match name {
            "openai" => ("gpt-4", "https://api.openai.com/v1", "/chat/completions"),
            _ => ("", "", ""),
        };

        let api_key = config.api_key.clone().unwrap_or(String::from(""));
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url.to_string());

        let chat_api = config
            .chat_api
            .clone()
            .unwrap_or_else(|| default_chat_api.to_string());

        let openai_config = OpenAICompatibleConfig {
            name: name.to_string(),
            api_key,
            model,
            base_url,
            chat_api,
            temperature: config.temperature,
            pricing_endpoint: config.pricing_endpoint.clone(),
        };

        Ok(Box::new(OpenAICompatibleBackend::new(openai_config)?))
    }
}

impl BackendFactory for OllamaBackend {
    fn create(config: &BackendConfig, name: &str) -> Result<Box<dyn LlmBackend>> {
        let (default_model, default_base_url) = ("llama3.1", "http://localhost:11434");

        let model = config
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url.to_string());

        let ollama_config = OllamaConfig {
            name: name.to_string(),
            model,
            base_url,
            temperature: config.temperature,
        };

        Ok(Box::new(OllamaBackend::new(ollama_config)?))
    }
}
pub fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    let backend_config = config
        .get_backend_config(backend_name)
        .ok_or_else(|| anyhow::anyhow!("Backend '{}' not found in config", backend_name))?;

    match backend_name {
        "mock" => Ok(Box::new(MockBackend::new())),
        #[cfg(feature = "together-ai")]
        "together_ai" => TogetherAiBackend::create(backend_config, backend_name),
        #[cfg(feature = "anthropic")]
        "anthropic" => AnthropicBackend::create(backend_config, backend_name),
        #[cfg(feature = "ollama")]
        "ollama" => OllamaBackend::create(backend_config, backend_name),
        #[cfg(feature = "openai-compatible")]
        name if matches!(name, "openai" | "groq") => {
            OpenAICompatibleBackend::create(backend_config, name)
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
