use anyhow::Result;
use async_trait::async_trait;

use crate::conversations::AgentEvent;
use crate::conversations::{Conversation, ToolCall};
use crate::tools::ToolRegistry;
use tokio::sync::mpsc::UnboundedSender;

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
    ) -> Result<LlmResponse> {
        let _ = event_tx;
        self.send_message_with_tools(conversation, tools).await
    }
}

#[cfg(feature = "anthropic")]
pub mod anthropic;
#[cfg(feature = "anthropic")]
pub mod backend_factory;
pub mod error;
pub mod llm_error;
pub mod mock;
#[cfg(feature = "openai-compatible")]
pub mod openai_compatible;
#[cfg(feature = "together-ai")]
pub mod together_ai;

#[cfg(feature = "anthropic")]
pub use self::anthropic::AnthropicBackend;
#[cfg(feature = "openai-compatible")]
pub use self::openai_compatible::OpenAICompatibleBackend;
#[cfg(feature = "together-ai")]
pub use self::together_ai::TogetherAiBackend;

pub use error::{BackendError, BackendResult};
pub use llm_error::LlmError;

#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicConfig;
pub use mock::MockBackend;
#[cfg(feature = "openai-compatible")]
pub use openai_compatible::OpenAICompatibleConfig;
#[cfg(feature = "together-ai")]
pub use together_ai::TogetherAiConfig;

pub mod executor;
pub use executor::RequestExecutor;

pub mod strategy;
pub use strategy::RetryStrategy;
