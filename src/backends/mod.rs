use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;

pub type StreamResponse = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<String>;
    async fn stream_message(&self, message: &str) -> Result<StreamResponse>;
    fn backend_name(&self) -> &'static str;
}

pub mod mock;
#[cfg(feature = "together-ai")]
pub mod together_ai;

pub use mock::MockBackend;
#[cfg(feature = "together-ai")]
pub use together_ai::{TogetherAiBackend, TogetherAiConfig};
