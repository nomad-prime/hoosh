use anyhow::Result;
use async_trait::async_trait;
use super::{LlmBackend, LlmResponse};
use crate::conversations::Conversation;
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

    fn backend_name(&self) -> &str {
        "mock"
    }

    fn model_name(&self) -> &str {
        "mock-model"
    }
}