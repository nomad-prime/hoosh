use crate::agent::AgentEvent;
use crate::backends::llm_error::LlmError;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;

pub struct RetryStrategy {
    pub max_attempts: u32,
    pub operation_name: String,
    pub event_tx: Option<UnboundedSender<AgentEvent>>,
}

impl RetryStrategy {
    pub fn new(
        max_attempts: u32,
        operation_name: String,
        event_tx: Option<UnboundedSender<AgentEvent>>,
    ) -> Self {
        Self {
            max_attempts,
            operation_name,
            event_tx,
        }
    }

    fn send_event(&self, event: AgentEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    pub async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T, LlmError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, LlmError>>,
    {
        let mut attempts = 0;
        let mut delay = Duration::from_secs(1);

        loop {
            match operation().await {
                Ok(result) => {
                    if attempts > 0 {
                        let success_message = format!(
                            "{} succeeded after {} attempts",
                            self.operation_name,
                            attempts + 1
                        );
                        self.send_event(AgentEvent::RetryEvent {
                            operation_name: self.operation_name.clone(),
                            attempt: attempts + 1,
                            max_attempts: self.max_attempts + 1,
                            message: success_message,
                            is_success: true,
                        });
                    }
                    return Ok(result);
                }
                Err(e) if e.is_retryable() && attempts < self.max_attempts => {
                    attempts += 1;

                    let actual_delay = if let LlmError::RateLimit {
                        retry_after: Some(seconds),
                        ..
                    } = &e
                    {
                        Duration::from_secs(*seconds)
                    } else {
                        delay
                    };

                    let retry_message = format!(
                        "Attempt {}/{} failed: {}. Retrying in {:?}...",
                        attempts,
                        self.max_attempts + 1,
                        e.short_message(),
                        actual_delay
                    );

                    self.send_event(AgentEvent::RetryEvent {
                        operation_name: self.operation_name.clone(),
                        attempt: attempts,
                        max_attempts: self.max_attempts + 1,
                        message: retry_message,
                        is_success: false,
                    });

                    sleep(actual_delay).await;
                    delay *= 2;
                }
                Err(e) => {
                    let final_message = if attempts > 0 {
                        format!(
                            "{} failed after {} attempts: {}",
                            self.operation_name,
                            attempts + 1,
                            e.short_message()
                        )
                    } else {
                        e.user_message()
                    };

                    self.send_event(AgentEvent::RetryEvent {
                        operation_name: self.operation_name.clone(),
                        attempt: attempts + 1,
                        max_attempts: self.max_attempts + 1,
                        message: final_message,
                        is_success: false,
                    });

                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_strategy_success_on_retry() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let attempt_count = AtomicU32::new(0);

        let strategy = RetryStrategy::new(3, "test_op".to_string(), Some(event_tx));

        let result = strategy
            .execute(|| async {
                let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(LlmError::RateLimit {
                        retry_after: None,
                        message: "rate limited".to_string(),
                    })
                } else {
                    Ok("success".to_string())
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }
        assert_eq!(events.len(), 3); // 2 retries + 1 success
    }

    #[tokio::test]
    async fn test_retry_strategy_non_retryable_error() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        let strategy = RetryStrategy::new(3, "test_op".to_string(), Some(event_tx));

        let result = strategy
            .execute(|| async {
                Err::<String, _>(LlmError::AuthenticationError {
                    message: "invalid key".to_string(),
                })
            })
            .await;

        assert!(result.is_err());

        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }
        assert_eq!(events.len(), 1); // Only error event
    }
}
