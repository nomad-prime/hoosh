use crate::backends::{
    AnthropicBackend, AnthropicConfig, LlmBackend, OpenAICompatibleBackend, OpenAICompatibleConfig,
    TogetherAiBackend, TogetherAiConfig,
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
        // Get provider-specific defaults from the backend name
        let (default_model, default_base_url) = match name {
            "openai" => ("gpt-4", "https://api.openai.com/v1"),
            "ollama" => ("llama3", "http://localhost:11434/v1"),
            "groq" => ("mixtral-8x7b-32768", "https://api.groq.com/openai/v1"),
            _ => ("", ""),
        };

        let api_key = config.api_key.clone().unwrap_or_else(|| {
            if name == "ollama" {
                "ollama".to_string()
            } else {
                String::new()
            }
        });
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| default_model.to_string());
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url.to_string());

        let openai_config = OpenAICompatibleConfig {
            name: name.to_string(),
            api_key,
            model,
            base_url,
            temperature: config.temperature,
        };

        Ok(Box::new(OpenAICompatibleBackend::new(openai_config)?))
    }
}
