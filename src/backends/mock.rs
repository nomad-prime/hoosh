use anyhow::Result;
use async_trait::async_trait;
use super::LlmBackend;

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

    fn backend_name(&self) -> &'static str {
        "mock"
    }
}