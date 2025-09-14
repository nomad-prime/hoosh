use anyhow::Result;
use async_trait::async_trait;
use futures_util::stream;
use super::{LlmBackend, LlmResponse, StreamResponse};
use crate::conversation::Conversation;
use crate::tools::ToolRegistry;

pub struct MockBackend;

impl MockBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LlmBackend for MockBackend {
    async fn send_message(&self, message: &str) -> Result<String> {
        Ok(format!("Mock response to: {}", message))
    }

    async fn stream_message(&self, message: &str) -> Result<StreamResponse> {
        let response = format!("Mock streaming response to: {}", message);
        let words: Vec<String> = response.split(' ').map(|s| s.to_string() + " ").collect();
        let stream = stream::iter(words.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }

    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        _tools: &ToolRegistry
    ) -> Result<LlmResponse> {
        // Mock implementation - just return a simple response
        if let Some(last_message) = conversation.messages.last() {
            if let Some(ref content) = last_message.content {
                Ok(LlmResponse::content_only(format!("Mock response with tools to: {}", content)))
            } else {
                Ok(LlmResponse::content_only("Mock response with tools".to_string()))
            }
        } else {
            Ok(LlmResponse::content_only("Mock response with tools (no messages)".to_string()))
        }
    }

    async fn stream_message_with_tools(
        &self,
        conversation: &Conversation,
        _tools: &ToolRegistry,
    ) -> Result<StreamResponse> {
        // Mock implementation - just stream a simple response
        let response = if let Some(last_message) = conversation.messages.last() {
            if let Some(ref content) = last_message.content {
                format!("Mock streaming response with tools to: {}", content)
            } else {
                "Mock streaming response with tools".to_string()
            }
        } else {
            "Mock streaming response with tools (no messages)".to_string()
        };

        let words: Vec<String> = response.split(' ').map(|s| s.to_string() + " ").collect();
        let stream = stream::iter(words.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }

    fn backend_name(&self) -> &'static str {
        "mock"
    }
}