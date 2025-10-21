use super::strategy::RetryStrategy;
use crate::backends::llm_error::LlmError;
use tokio::sync::mpsc::UnboundedSender;
use crate::conversations::AgentEvent;

pub struct RequestExecutor {
    retry_strategy: RetryStrategy,
}

impl RequestExecutor {
    pub fn new(
        max_attempts: u32,
        operation_name: String,
        event_tx: Option<UnboundedSender<AgentEvent>>,
    ) -> Self {
        Self {
            retry_strategy: RetryStrategy::new(max_attempts, operation_name, event_tx),
        }
    }

    pub async fn execute<F, Fut, T>(&self, operation: F) -> Result<T, LlmError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, LlmError>>,
    {
        self.retry_strategy.execute(operation).await
    }
}

impl Clone for RequestExecutor {
    fn clone(&self) -> Self {
        // Create a new executor with the same configuration
        // Note: This creates a new event channel, so events won't be sent
        // For proper cloning with event forwarding, use the new() constructor
        Self {
            retry_strategy: RetryStrategy::new(
                self.retry_strategy.max_attempts,
                self.retry_strategy.operation_name.clone(),
                None,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_delegates_to_strategy() {
        let executor = RequestExecutor::new(3, "test".to_string(), None);

        let result = executor
            .execute(|| async { Ok("test".to_string()) })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test");
    }
}
