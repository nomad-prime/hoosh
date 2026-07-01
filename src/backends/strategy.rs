use crate::agent::AgentEvent;
use crate::backends::llm_error::LlmError;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;

/// Format a retry delay in a way that reads naturally in the TUI status line.
/// Whole seconds render as `2s`; minute-scale waits as `2m 5s`; sub-second
/// values as `500ms`.
fn format_duration(d: Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms < 1000 {
        return format!("{}ms", total_ms);
    }
    let total_secs = d.as_secs();
    if total_secs < 60 {
        return format!("{}s", total_secs);
    }
    let m = total_secs / 60;
    let s = total_secs % 60;
    if s == 0 {
        format!("{}m", m)
    } else {
        format!("{}m {}s", m, s)
    }
}

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
        Fut: Future<Output = Result<T, LlmError>>,
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
                            max_attempts: self.max_attempts,
                            message: success_message,
                            is_success: true,
                        });
                    }
                    return Ok(result);
                }
                Err(e) if e.is_retryable() && attempts + 1 < self.max_attempts => {
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

                    // Display the number of the attempt we're about to make,
                    // not the count of failures so far. With max_attempts=3 the
                    // user now sees 2/3 → 3/3 with no apparent gap.
                    let retry_message = format!(
                        "{}. Retrying in {}... (Attempt {}/{})",
                        e.short_message(),
                        format_duration(actual_delay),
                        attempts + 1,
                        self.max_attempts
                    );

                    self.send_event(AgentEvent::RetryEvent {
                        operation_name: self.operation_name.clone(),
                        attempt: attempts,
                        max_attempts: self.max_attempts,
                        message: retry_message,
                        is_success: false,
                    });

                    sleep(actual_delay).await;
                    delay *= 2;
                }
                Err(e) => {
                    // Only send retry event if we actually attempted retries
                    if attempts > 0 {
                        let final_message = format!(
                            "{} failed after {} attempts: {}",
                            self.operation_name,
                            attempts + 1,
                            e.short_message()
                        );

                        // Always terminal: report the final attempt so the UI settles.
                        self.send_event(AgentEvent::RetryEvent {
                            operation_name: self.operation_name.clone(),
                            attempt: self.max_attempts,
                            max_attempts: self.max_attempts,
                            message: final_message,
                            is_success: false,
                        });
                    }

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
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3); // Verify 3 total attempts

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
        // No retry events should be sent for non-retryable errors on first attempt
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn format_duration_renders_natural_units() {
        assert_eq!(format_duration(Duration::from_millis(250)), "250ms");
        assert_eq!(format_duration(Duration::from_secs(1)), "1s");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
    }

    #[tokio::test]
    async fn retry_message_counts_the_upcoming_attempt() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let attempt_count = AtomicU32::new(0);

        let strategy = RetryStrategy::new(3, "op".to_string(), Some(event_tx));
        let _ = strategy
            .execute(|| async {
                attempt_count.fetch_add(1, Ordering::SeqCst);
                Err::<String, _>(LlmError::RateLimit {
                    retry_after: None,
                    message: "rate limited".to_string(),
                })
            })
            .await;

        // 2 in-flight retry events (attempts 2/3 and 3/3) + 1 terminal failure
        let mut msgs = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            if let AgentEvent::RetryEvent { message, .. } = event {
                msgs.push(message);
            }
        }
        assert_eq!(msgs.len(), 3, "got {msgs:?}");
        assert!(
            msgs[0].contains("Attempt 2/3"),
            "first retry should announce upcoming attempt #2, got {:?}",
            msgs[0]
        );
        assert!(
            msgs[1].contains("Attempt 3/3"),
            "second retry should announce upcoming attempt #3, got {:?}",
            msgs[1]
        );
        assert!(
            msgs[2].contains("failed after 3 attempts"),
            "terminal message should report 3 attempts, got {:?}",
            msgs[2]
        );
    }

    #[tokio::test]
    async fn test_retry_strategy_respects_max_attempts() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let attempt_count = AtomicU32::new(0);

        let strategy = RetryStrategy::new(3, "test_op".to_string(), Some(event_tx));

        let result = strategy
            .execute(|| async {
                attempt_count.fetch_add(1, Ordering::SeqCst);
                Err::<String, _>(LlmError::RateLimit {
                    retry_after: None,
                    message: "rate limited".to_string(),
                })
            })
            .await;

        assert!(result.is_err());
        // With max_attempts = 3, should only attempt exactly 3 times
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);

        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }
        // 2 retry events + 1 final failure event
        assert_eq!(events.len(), 3);
    }
}
