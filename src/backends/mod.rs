use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;

use crate::conversation::{Conversation, ToolCall};
use crate::tools::ToolRegistry;

pub type StreamResponse = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

#[derive(Debug)]
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

    async fn stream_message(&self, message: &str) -> Result<StreamResponse>;

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse>;

    async fn stream_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<StreamResponse>;

    fn backend_name(&self) -> &'static str;
}

pub mod mock;
#[cfg(feature = "together-ai")]
pub mod together_ai;

pub use mock::MockBackend;
#[cfg(feature = "together-ai")]
pub use together_ai::{TogetherAiBackend, TogetherAiConfig};
