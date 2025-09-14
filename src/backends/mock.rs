use anyhow::Result;
use async_trait::async_trait;
use futures_util::stream;
use super::{LlmBackend, StreamResponse};

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

    fn backend_name(&self) -> &'static str {
        "mock"
    }
}