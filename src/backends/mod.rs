use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<String>;
    fn backend_name(&self) -> &'static str;
}

pub mod mock;

pub use mock::MockBackend;