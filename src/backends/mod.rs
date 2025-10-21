use anyhow::Result;
use async_trait::async_trait;

use crate::conversations::{Conversation, ToolCall};
use crate::tools::ToolRegistry;

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl LlmResponse {
    pub fn content_only(content: String) -> Self {
        Self {
            content: Some(content),
            tool_calls: None,
        }
    }

    pub fn with_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content,
            tool_calls: Some(tool_calls),
        }
    }
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<String>;

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse>;

    fn backend_name(&self) -> &str;
    fn model_name(&self) -> &str;
}

#[cfg(feature = "anthropic")]
pub mod anthropic;
pub mod llm_error;
pub mod mock;
#[cfg(feature = "openai-compatible")]
pub mod openai_compatible;
pub mod retry;
#[cfg(feature = "together-ai")]
pub mod together_ai;

pub use llm_error::LlmError;
pub use retry::retry_with_backoff;

#[cfg(feature = "anthropic")]
pub use anthropic::{AnthropicBackend, AnthropicConfig};
pub use mock::MockBackend;
#[cfg(feature = "openai-compatible")]
pub use openai_compatible::{OpenAICompatibleBackend, OpenAICompatibleConfig};
#[cfg(feature = "together-ai")]
pub use together_ai::{TogetherAiBackend, TogetherAiConfig};

pub mod executor;
pub use executor::RequestExecutor;

pub mod strategy;
pub use strategy::RetryStrategy;
