use anyhow::Result;
use async_trait::async_trait;

use crate::conversations::{AgentEvent, Conversation, ToolCall};
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
    // Keep existing methods but add new ones with event support
    async fn send_message(&self, message: &str) -> Result<String>;

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse>;

    async fn send_message_with_events(
        &self,
        message: &str,
        event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<String> {
        // Default implementation - backends should override for better experience
        let result = self.send_message(message).await;
        if let Err(ref e) = result {
            let _ = event_tx.send(AgentEvent::Error(format!("Error: {}", e)));
        }
        result
    }

    async fn send_message_with_tools_and_events(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
        event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<LlmResponse> {
        // Default implementation - backends should override for better experience
        let result = self.send_message_with_tools(conversation, tools).await;
        if let Err(ref e) = result {
            let _ = event_tx.send(AgentEvent::Error(format!("Error: {}", e)));
        }
        result
    }

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
