# Phase 1: Retry Logic and Graceful Error Handling for LLM Backends (Event-Driven User Experience)

## Overview
This document outlines Phase 1 of the graceful LLM error handling implementation using the existing event-driven architecture. This phase implements automatic retry mechanisms for transient errors (rate limits, server errors) and improves error messages displayed to users through the existing AgentEvent system.

## Goals for Phase 1
1. Implement automatic retry with exponential backoff for HTTP 429 (rate limit) errors
2. Implement automatic retry with exponential backoff for HTTP 5xx (server) errors
3. Provide clear, actionable error messages to users when errors occur
4. **Use existing event system for transparent user feedback**
5. Preserve existing functionality without breaking changes

## Implementation Tasks

### 1. Define LLM Error Types

**File to Create**: `src/backends/llm_error.rs`
**Description**: Create custom error types for better error classification
```rust
#[derive(Debug, Clone)]
pub enum LlmError {
    RateLimit { retry_after: Option<u64>, message: String },
    ServerError { status: u16, message: String },
    AuthenticationError { message: String },
    NetworkError { message: String },
    Other { message: String },
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, LlmError::RateLimit { .. } | LlmError::ServerError { .. })
    }
    
    pub fn user_message(&self) -> String {
        match self {
            LlmError::RateLimit { message, .. } => format!("Rate limit: {}", message),
            LlmError::ServerError { status, message } => {
                format!("Server error ({}): {}", status, message)
            },
            LlmError::AuthenticationError { message } => {
                format!("Authentication error: {}", message)
            },
            LlmError::NetworkError { message } => {
                format!("Network error: {}", message)
            },
            LlmError::Other { message } => {
                format!("Error: {}", message)
            },
        }
    }
    
    pub fn short_message(&self) -> String {
        match self {
            LlmError::RateLimit { .. } => "Rate limit hit".to_string(),
            LlmError::ServerError { status, .. } => format!("Server error ({})", status),
            LlmError::AuthenticationError { .. } => "Authentication error".to_string(),
            LlmError::NetworkError { .. } => "Network error".to_string(),
            LlmError::Other { .. } => "Error occurred".to_string(),
        }
    }
}
```
**Validation**: Run `bash: cargo check`

### 2. Implement Retry Utility with Exponential Backoff and Event Notifications

**File to Create**: `src/backends/retry.rs`
**Description**: Create a utility function for retrying operations with exponential backoff and event notifications
```rust
use std::time::Duration;
use tokio::time::sleep;
use crate::backends::llm_error::LlmError;
use crate::conversations::AgentEvent;

pub struct RetryResult<T> {
    pub result: Result<T, LlmError>,
    pub attempts: u32,
    pub retry_events: Vec<AgentEvent>,
}

pub async fn retry_with_backoff<F, Fut, T>(
    mut operation: F,
    max_retries: u32,
    operation_name: &str,  // For user messages
    event_sender: tokio::sync::mpsc::UnboundedSender<AgentEvent>,  // Event channel
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
                    let success_message = format!("✅ {} succeeded after {} attempts", operation_name, attempts + 1);
                    let event = AgentEvent::Info(success_message.clone());
                    let _ = event_sender.send(event.clone());
                    retry_events.push(event);
                }
                return RetryResult {
                    result: Ok(result),
                    attempts: attempts + 1,
                    retry_events,
                };
            },
            Err(e) if e.is_retryable() && attempts < max_retries => {
                attempts += 1;
                
                // For rate limits, use provided delay if available
                let actual_delay = if let LlmError::RateLimit { retry_after: Some(seconds), .. } = &e {
                    Duration::from_secs(*seconds)
                } else {
                    delay
                };
                
                let retry_message = format!(
                    "⏳ Attempt {}/{} failed: {}. Retrying in {:?}...",
                    attempts, max_retries + 1, e.short_message(), actual_delay
                );
                
                let event = AgentEvent::Info(retry_message.clone());
                let _ = event_sender.send(event.clone());
                retry_events.push(event);
                
                sleep(actual_delay).await;
                delay *= 2; // Exponential backoff
            }
            Err(e) => {
                let final_message = if attempts > 0 {
                    format!("❌ {} failed after {} attempts: {}", operation_name, attempts + 1, e.user_message())
                } else {
                    format!("❌ {}: {}", operation_name, e.user_message())
                };
                
                let event = AgentEvent::Error(final_message.clone());
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
```
**Validation**: Run `bash: cargo check`

### 3. Update Backend Trait for Better Error Handling

**File to Modify**: `src/backends/mod.rs`
**Description**: Add the new error types and retry utility to the backend module
- Add `pub mod llm_error;` and `pub mod retry;` to module exports
- Update imports as needed
**Validation**: Run `bash: cargo check`

### 4. Enhance OpenAI Compatible Backend Error Handling

**File to Modify**: `src/backends/openai_compatible.rs`
**Description**: Update the OpenAI compatible backend to use the new error handling:
1. Import the new error types: `use crate::backends::llm_error::LlmError;`
2. Modify error handling in `send_message` and `send_message_with_tools`:
   - Parse HTTP 429 status codes and extract retry-after header if present
   - Parse HTTP 5xx status codes as server errors
   - Convert network errors to appropriate error types
   - Wrap the actual HTTP request in the retry utility

Example modification for error handling:
```rust
// Modify function signatures to accept event sender
async fn send_message_with_retry(
    &self,
    message: &str,
    event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
) -> Result<String> {
    let retry_result = retry_with_backoff(
        || async {
            // ... existing HTTP request code ...
            
            if !response.status().is_success() {
                let status = response.status().as_u16();
                let error_text = response.text().await.unwrap_or_default();
                
                match status {
                    429 => {
                        // Try to parse retry-after header
                        let retry_after = response.headers()
                            .get("retry-after")
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok());
                        return Err(LlmError::RateLimit { 
                            retry_after,
                            message: error_text.clone()
                        });
                    },
                    500..=599 => {
                        return Err(LlmError::ServerError { 
                            status, 
                            message: error_text 
                        });
                    },
                    401 | 403 => {
                        return Err(LlmError::AuthenticationError { 
                            message: error_text 
                        });
                    },
                    _ => {
                        return Err(LlmError::Other { 
                            message: format!("API error {}: {}", status, error_text) 
                        });
                    }
                }
            }
            
            // ... existing success handling ...
        },
        3, // max retries
        "OpenAI API request",
        event_tx,
    ).await;
    
    retry_result.result.map_err(|e| anyhow::anyhow!(e.user_message()))
}
```
**Validation**: Run `bash: cargo check`

### 5. Update TUI Event Loop for Transparent Error Handling

**File to Modify**: `src/tui/event_loop.rs`
**Description**: Update the event loop to use the enhanced backends with event reporting:
- Pass the existing event_tx to backend calls
- Handle the retry events that are generated
- Ensure all retry messages are displayed to the user

Example modification:
```rust
// When calling backend methods, pass the existing event_tx
// The retry utility will automatically send events through this channel
let retry_result = retry_with_backoff(
    || async {
        // Your backend call here
        backend.send_message_with_tools(&conversation, &tools).await
            .map_err(|e| LlmError::Other { message: e.to_string() }) // Convert as needed
    },
    3,
    "LLM request",
    event_tx.clone(), // Use existing event channel
).await;

// The retry events are automatically handled by the existing event loop
// No additional handling needed here - they'll appear as Info/Error messages

match retry_result.result {
    Ok(response) => {
        // Handle successful response
    }
    Err(e) => {
        // Error already displayed through events, but you might want to add final handling
    }
}
```
**Validation**: Run `bash: cargo check`

### 6. Enhance Anthropic Backend Error Handling

**File to Modify**: `src/backends/anthropic.rs`
**Description**: Apply the same error handling improvements:
1. Import the new error types
2. Parse Anthropic-specific error responses
3. Implement retry logic for rate limits and server errors
4. Provide user-friendly error messages
5. Use the retry utility with event notifications
**Validation**: Run `bash: cargo check`

### 7. Enhance TogetherAI Backend Error Handling

**File to Modify**: `src/backends/together_ai.rs`
**Description**: Apply the same error handling improvements:
1. Import the new error types
2. Parse TogetherAI-specific error responses
3. Implement retry logic for rate limits and server errors
4. Provide user-friendly error messages
5. Use the retry utility with event notifications
**Validation**: Run `bash: cargo check`

### 8. Update Backend Trait Methods to Accept Event Sender

**File to Modify**: `src/backends/mod.rs`
**Description**: Update the LlmBackend trait to accept an event sender for transparent messaging:
```rust
#[async_trait]
pub trait LlmBackend: Send + Sync {
    // Keep existing methods but add new ones with event support
    async fn send_message(&self, message: &str) -> Result<String>;
    
    async fn send_message_with_tools(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
    ) -> Result<LlmResponse>;
    
    // NEW: Methods that support event notifications
    async fn send_message_with_events(
        &self,
        message: &str,
        event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<String> {
        // Default implementation - backends should override for better experience
        let result = self.send_message(message).await;
        if let Err(ref e) = result {
            let _ = event_tx.send(AgentEvent::Error(format!("Error: {}", e)));
        }
        result
    }
    
    async fn send_message_with_tools_and_events(
        &self,
        conversation: &Conversation,
        tools: &ToolRegistry,
        event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<LlmResponse> {
        // Default implementation - backends should override for better experience
        let result = self.send_message_with_tools(conversation, tools).await;
        if let Err(ref e) = result {
            let _ = event_tx.send(AgentEvent::Error(format!("Error: {}", e)));
        }
        result
    }
    
    fn backend_name(&self) -> &str;
    fn model_name(&self) -> &str;
}
```
**Validation**: Run `bash: cargo check`

### 9. Update Backend Implementations to Override Event Methods

**File to Modify**: Each backend file (`openai_compatible.rs`, `anthropic.rs`, `together_ai.rs`)
**Description**: Override the new event-based methods to provide transparent retry feedback:
```rust
// In each backend implementation
async fn send_message_with_events(
    &self,
    message: &str,
    event_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
) -> Result<String> {
    let retry_result = retry_with_backoff(
        || self.send_message_attempt(message),
        3,
        &format!("{} request", self.backend_name()),
        event_tx.clone(),
    ).await;
    
    retry_result.result.map_err(|e| anyhow::anyhow!(e.user_message()))
}
```
**Validation**: Run `bash: cargo check`

### 10. Create Unit Tests for Retry Logic

**File to Create**: `src/backends/retry_tests.rs` (or add to existing tests)
**Description**: Create tests to verify the retry logic works correctly:
- Test that retryable errors are retried up to max attempts
- Test that non-retryable errors are returned immediately
- Test exponential backoff timing
- Test rate limit retry-after header handling
- Test that events are properly sent through the channel
**Validation**: Run `bash: cargo test`

### 11. Manual Testing with Event-Driven User Experience

**Description**: Manually test the event-driven error handling:
1. Intentionally trigger rate limits (if possible) or simulate them
2. Test network error scenarios
3. Verify retry behavior is clearly shown to users through events:
   - "⏳ Attempt 1/4 failed: Rate limit hit. Retrying in 2s..." (as Info events)
   - "⏳ Attempt 2/4 failed: Rate limit hit. Retrying in 4s..." (as Info events)
   - "✅ LLM request succeeded after 3 attempts" (as Info event)
4. Check that final failures are shown as Error events
5. Ensure existing functionality still works

### 12. Documentation Update

**File to Update**: README.md or appropriate documentation
**Description**: Document the new event-driven error handling features:
- Automatic retry for rate limits and server errors
- Real-time feedback through the event system
- What users can expect to see when errors occur
**Validation**: Manual review

## Expected User Experience

With this implementation using the event system, users will see messages like:

```
User: Can you help me debug this code?
Assistant: [thinking...]

⏳ Attempt 1/4 failed: Rate limit hit. Retrying in 2s...
⏳ Attempt 2/4 failed: Rate limit hit. Retrying in 4s...
✅ LLM request succeeded after 3 attempts

Assistant: Of course! I can help you debug that code...
```

Or in case of final failure:

```
User: Analyze this large dataset
Assistant: [thinking...]

⏳ Attempt 1/3 failed: Server error (503). Retrying in 1s...
⏳ Attempt 2/3 failed: Server error (503). Retrying in 2s...
⏳ Attempt 3/3 failed: Server error (503). Retrying in 4s...
❌ LLM request failed after 4 attempts: Server error (503): Service temporarily unavailable
```

## Benefits of Event-Driven Approach

1. **Consistent Architecture**: Uses the existing AgentEvent system
2. **Automatic Integration**: Events are automatically handled by the TUI
3. **Clean Separation**: Backends don't need to know about UI details
4. **Transparent Experience**: Users see exactly what's happening during retries
5. **Professional Experience**: Clear feedback builds user trust

This approach integrates seamlessly with the existing hoosh architecture while providing the transparent error handling users need.