use anyhow::Result;
use async_trait::async_trait;

use crate::agent::AgentEvent;
use crate::agent::{Conversation, ToolCall};
use crate::tools::ToolRegistry;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
}

impl LlmResponse {
    pub fn content_only(content: String) -> Self {
        Self {
            content: Some(content),
            tool_calls: None,
            input_tokens: None,
            output_tokens: None,
        }
    }

    pub fn with_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content,
            tool_calls: Some(tool_calls),
            input_tokens: None,
            output_tokens: None,
        }
    }

    pub fn with_tokens(mut self, input_tokens: usize, output_tokens: usize) -> Self {
        self.input_tokens = Some(input_tokens);
        self.output_tokens = Some(output_tokens);
        self
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct TokenPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
}

impl TokenPricing {
    pub fn calculate_cost(&self, input_tokens: usize, output_tokens: usize) -> f64 {
        (input_tokens as f64 * self.input_per_million / 1_000_000.0)
            + (output_tokens as f64 * self.output_per_million / 1_000_000.0)
    }
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<String>;

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse, LlmError>;

    fn backend_name(&self) -> &str;
    fn model_name(&self) -> &str;

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    fn pricing(&self) -> Option<TokenPricing> {
        None
    }

    async fn send_message_with_events(
        &self,
        message: &str,
        event_tx: Option<UnboundedSender<AgentEvent>>,
    ) -> Result<String> {
        let _ = event_tx;
        self.send_message(message).await
    }

    async fn send_message_with_tools_and_events(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
        event_tx: Option<UnboundedSender<AgentEvent>>,
    ) -> Result<LlmResponse, LlmError> {
        let _ = event_tx;
        self.send_message_with_tools(conversation, tools).await
    }
}

#[cfg(feature = "anthropic")]
pub mod anthropic;
#[cfg(feature = "anthropic")]
pub mod backend_factory;
pub mod llm_error;
pub mod mock;
pub mod ollama;
#[cfg(feature = "openai-compatible")]
pub mod openai_compatible;
#[cfg(feature = "together-ai")]
pub mod together_ai;

#[cfg(feature = "anthropic")]
pub use self::anthropic::AnthropicBackend;
pub use self::ollama::OllamaBackend;
#[cfg(feature = "openai-compatible")]
pub use self::openai_compatible::OpenAICompatibleBackend;
#[cfg(feature = "together-ai")]
pub use self::together_ai::TogetherAiBackend;

pub use llm_error::LlmError;

#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicConfig;
pub use mock::MockBackend;
pub use ollama::OllamaConfig;
#[cfg(feature = "openai-compatible")]
pub use openai_compatible::OpenAICompatibleConfig;
#[cfg(feature = "together-ai")]
pub use together_ai::TogetherAiConfig;

pub mod executor;
pub use executor::RequestExecutor;

pub mod strategy;
pub use strategy::RetryStrategy;
