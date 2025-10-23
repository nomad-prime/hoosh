use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Backend '{backend}' not found in configuration")]
    BackendNotFound { backend: String },

    #[error("API request failed: {message}")]
    RequestFailed {
        message: String,
        status: Option<u16>,
    },

    #[error("Rate limit exceeded. Retry after {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Invalid API key for backend '{backend}'")]
    InvalidApiKey { backend: String },

    #[error("Model '{model}' not available for backend '{backend}'")]
    ModelNotAvailable { backend: String, model: String },

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Backend configuration error: {0}")]
    ConfigurationError(String),

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Server error (status {status}): {message}")]
    ServerError { status: u16, message: String },

    #[error("Invalid response from backend: {0}")]
    InvalidResponse(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

pub type BackendResult<T> = Result<T, BackendError>;

impl BackendError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            BackendError::RateLimitExceeded { .. }
                | BackendError::ServerError {
                    status: 500..=599,
                    ..
                }
                | BackendError::NetworkError(_)
                | BackendError::Timeout(_)
        )
    }

    pub fn user_message(&self) -> String {
        match self {
            BackendError::RateLimitExceeded { retry_after } => {
                format!("Rate limit: Please retry after {} seconds", retry_after)
            }
            BackendError::InvalidApiKey { backend } => {
                format!("Invalid API key for backend '{}'", backend)
            }
            BackendError::ModelNotAvailable { backend, model } => {
                format!("Model '{}' not available for backend '{}'", model, backend)
            }
            BackendError::NetworkError(msg) => format!("Network error: {}", msg),
            BackendError::ServerError { status, message } => {
                format!("Server error ({}): {}", status, message)
            }
            BackendError::AuthenticationError(msg) => format!("Authentication error: {}", msg),
            BackendError::Timeout(msg) => format!("Request timeout: {}", msg),
            _ => self.to_string(),
        }
    }
}
