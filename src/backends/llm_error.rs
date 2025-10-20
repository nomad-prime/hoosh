#[derive(Debug, Clone)]
pub enum LlmError {
    RateLimit {
        retry_after: Option<u64>,
        message: String,
    },
    ServerError {
        status: u16,
        message: String,
    },
    AuthenticationError {
        message: String,
    },
    NetworkError {
        message: String,
    },
    Other {
        message: String,
    },
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LlmError::RateLimit { .. }
                | LlmError::ServerError { .. }
                | LlmError::NetworkError { .. }
        )
    }

    pub fn user_message(&self) -> String {
        match self {
            LlmError::RateLimit { message, .. } => Self::extract_error_message(message)
                .map(|m| format!("Rate limit: {}", m))
                .unwrap_or_else(|| format!("Rate limit: {}", message)),
            LlmError::ServerError { status, message } => {
                let error_msg = Self::extract_error_message(message).unwrap_or(message.clone());
                format!("Server error ({}): {}", status, error_msg)
            }
            LlmError::AuthenticationError { message } => {
                let error_msg = Self::extract_error_message(message).unwrap_or(message.clone());
                format!("Authentication error: {}", error_msg)
            }
            LlmError::NetworkError { message } => {
                format!("Network error: {}", message)
            }
            LlmError::Other { message } => {
                format!("Error: {}", message)
            }
        }
    }

    fn extract_error_message(raw: &str) -> Option<String> {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(raw) {
            json.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        } else {
            None
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
