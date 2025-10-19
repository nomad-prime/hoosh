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