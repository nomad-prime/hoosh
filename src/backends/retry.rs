use crate::backends::llm_error::LlmError;
use crate::conversations::AgentEvent;
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryResult<T> {
    pub result: Result<T, LlmError>,
    pub attempts: u32,
    pub retry_events: Vec<AgentEvent>,
}

pub async fn retry_with_backoff<F, Fut, T>(
    mut operation: F,
    max_retries: u32,
    operation_name: &str, // For user messages
    event_sender: tokio::sync::mpsc::UnboundedSender<AgentEvent>, // Event channel
) -> RetryResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, LlmError>>,
{
    let mut attempts = 0;
    let mut delay = Duration::from_secs(1);
    let mut retry_events = Vec::new();

    loop {
        match operation().await {
            Ok(result) => {
                if attempts > 0 {
                    let success_message = format!(
                        "{} succeeded after {} attempts",
                        operation_name,
                        attempts + 1
                    );
                    let event = AgentEvent::RetryEvent {
                        operation_name: operation_name.to_string(),
                        attempt: attempts + 1,
                        max_attempts: max_retries + 1,
                        message: success_message.clone(),
                        is_success: true,
                    };
                    let _ = event_sender.send(event.clone());
                    retry_events.push(event);
                }
                return RetryResult {
                    result: Ok(result),
                    attempts: attempts + 1,
                    retry_events,
                };
            }
            Err(e) if e.is_retryable() && attempts < max_retries => {
                attempts += 1;

                // For rate limits, use provided delay if available
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
                    "⏳ Attempt {}/{} failed: {}. Retrying in {:?}...",
                    attempts,
                    max_retries + 1,
                    e.short_message(),
                    actual_delay
                );

                let event = AgentEvent::RetryEvent {
                    operation_name: operation_name.to_string(),
                    attempt: attempts,
                    max_attempts: max_retries + 1,
                    message: retry_message.clone(),
                    is_success: false,
                };
                let _ = event_sender.send(event.clone());
                retry_events.push(event);

                sleep(actual_delay).await;
                delay *= 2; // Exponential backoff
            }
            Err(e) => {
                let final_message = if attempts > 0 {
                    format!(
                        "❌ {} failed after {} attempts: {}",
                        operation_name,
                        attempts + 1,
                        e.user_message()
                    )
                } else {
                    format!("❌ {}: {}", operation_name, e.user_message())
                };

                let event = AgentEvent::RetryEvent {
                    operation_name: operation_name.to_string(),
                    attempt: attempts + 1,
                    max_attempts: max_retries + 1,
                    message: final_message.clone(),
                    is_success: false,
                };
                let _ = event_sender.send(event.clone());
                retry_events.push(event);

                return RetryResult {
                    result: Err(e),
                    attempts: attempts + 1,
                    retry_events,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::llm_error::LlmError;
    use crate::conversations::AgentEvent;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;
    use tokio::time::Instant;

    #[tokio::test]
    async fn test_retryable_error_retried() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        // Counter to track how many times the operation is called
        let attempt_count = AtomicU32::new(0);

        let retry_result = retry_with_backoff(
            || async {
                let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    // First two attempts fail with retryable error
                    Err(LlmError::RateLimit {
                        retry_after: None,
                        message: "test rate limit".to_string(),
                    })
                } else {
                    // Third attempt succeeds
                    Ok("success".to_string())
                }
            },
            3, // max retries
            "Test operation",
            event_tx,
        )
        .await;

        // Should succeed on third attempt
        assert!(retry_result.result.is_ok());
        assert_eq!(retry_result.result.unwrap(), "success");
        assert_eq!(retry_result.attempts, 3);

        // Check that we got the expected events
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        // Should have 2 retry events and 1 success event
        assert_eq!(events.len(), 3);
        // Check that events contain expected messages
        for event in &events {
            if let AgentEvent::RetryEvent { message, .. } = event {
                assert!(message.contains("Attempt") || message.contains("succeeded"));
            }
        }
    }

    #[tokio::test]
    async fn test_non_retryable_error_not_retried() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        let attempt_count = AtomicU32::new(0);

        let retry_result: RetryResult<String> = retry_with_backoff(
            || async {
                attempt_count.fetch_add(1, Ordering::SeqCst);
                // Non-retryable error
                Err(LlmError::AuthenticationError {
                    message: "invalid key".to_string(),
                })
            },
            3, // max retries
            "Test operation",
            event_tx,
        )
        .await;

        // Should fail immediately without retries
        assert!(retry_result.result.is_err());
        assert_eq!(retry_result.attempts, 1);

        // Check that we got the expected events
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        // Should have 1 error event
        assert_eq!(events.len(), 1);
        // Now we expect a RetryEvent instead of Error
        assert!(matches!(events[0], AgentEvent::RetryEvent { .. }));
    }

    #[tokio::test]
    async fn test_max_retries_reached() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        let attempt_count = AtomicU32::new(0);

        let retry_result: RetryResult<String> = retry_with_backoff(
            || async {
                attempt_count.fetch_add(1, Ordering::SeqCst);
                // Always fail with retryable error
                Err(LlmError::ServerError {
                    status: 503,
                    message: "service unavailable".to_string(),
                })
            },
            2, // max retries
            "Test operation",
            event_tx,
        )
        .await;

        // Should fail after max retries
        assert!(retry_result.result.is_err());
        assert_eq!(retry_result.attempts, 3); // 1 initial + 2 retries

        // Check that we got the expected events
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        // Should have 2 retry events and 1 final error event
        assert_eq!(events.len(), 3);
        // First two should be retry attempts
        for i in 0..2 {
            if let AgentEvent::RetryEvent { message, .. } = &events[i] {
                assert!(message.contains("Attempt"));
            } else {
                panic!("Expected RetryEvent event");
            }
        }
        // Last should be error (now a RetryEvent)
        assert!(matches!(events[2], AgentEvent::RetryEvent { .. }));
    }

    #[tokio::test]
    async fn test_retry_after_header_handling() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        let attempt_count = AtomicU32::new(0);

        let start_time = Instant::now();

        let retry_result = retry_with_backoff(
            || async {
                let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First attempt fails with rate limit and retry-after
                    Err(LlmError::RateLimit {
                        retry_after: Some(1), // 1 second
                        message: "rate limited".to_string(),
                    })
                } else {
                    // Second attempt succeeds
                    Ok("success".to_string())
                }
            },
            3, // max retries
            "Test operation",
            event_tx,
        )
        .await;

        let elapsed = start_time.elapsed();

        // Should succeed on second attempt
        assert!(retry_result.result.is_ok());
        assert_eq!(retry_result.attempts, 2);

        // Should have waited at least 1 second due to retry-after
        assert!(elapsed >= Duration::from_secs(1));

        // Check that we got the expected events
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        // Should have 1 retry event and 1 success event
        assert_eq!(events.len(), 2);
        // First should be retry attempt with correct delay info
        if let AgentEvent::RetryEvent { message, .. } = &events[0] {
            assert!(message.contains("Retrying in") && message.contains("1s"));
        }
        // Second should be success
        assert!(matches!(events[1], AgentEvent::RetryEvent { .. }));
    }
}
