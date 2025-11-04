use super::strategy::RetryStrategy;
use crate::agent::AgentEvent;
use crate::backends::llm_error::LlmError;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct RequestExecutor {
    max_attempts: u32,
    operation_name: String,
}

impl RequestExecutor {
    pub fn new(max_attempts: u32, operation_name: String) -> Self {
        Self {
            max_attempts,
            operation_name,
        }
    }

    pub async fn execute<F, Fut, T>(
        &self,
        operation: F,
        event_tx: Option<UnboundedSender<AgentEvent>>,
    ) -> Result<T, LlmError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, LlmError>>,
    {
        let strategy = RetryStrategy::new(self.max_attempts, self.operation_name.clone(), event_tx);
        strategy.execute(operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_delegates_to_strategy() {
        let executor = RequestExecutor::new(3, "test".to_string());

        let result = executor
            .execute(|| async { Ok("test".to_string()) }, None)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test");
    }
}
