use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Operation rejected: {reason}")]
    UserRejected { reason: String },

    #[error("Tool '{tool}' not found in registry")]
    ToolNotFound { tool: String },

    #[error("Invalid arguments for tool '{tool}': {message}")]
    InvalidArguments { tool: String, message: String },

    #[error("Tool execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("Permission denied: {tool}")]
    PermissionDenied { tool: String },

    #[error("Path security violation: {message}")]
    SecurityViolation { message: String },

    #[error("Timeout executing tool '{tool}' after {seconds} seconds")]
    Timeout { tool: String, seconds: u64 },

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Failed to read file: {path}")]
    ReadFailed { path: PathBuf },

    #[error("Failed to write file: {path}")]
    WriteFailed { path: PathBuf },

    #[error("Failed to edit file: {message}")]
    EditFailed { message: String },

    #[error("Invalid command: {message}")]
    InvalidCommand { message: String },

    #[error("Schema validation failed for tool '{tool}': {message}")]
    SchemaValidationFailed { tool: String, message: String },

    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(serde_json::Error),
}

impl ToolError {
    pub fn user_rejected(reason: impl Into<String>) -> Self {
        Self::UserRejected {
            reason: reason.into(),
        }
    }

    pub fn tool_not_found(tool: impl Into<String>) -> Self {
        Self::ToolNotFound { tool: tool.into() }
    }

    pub fn invalid_arguments(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidArguments {
            tool: tool.into(),
            message: message.into(),
        }
    }

    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
        }
    }

    pub fn permission_denied(tool: impl Into<String>) -> Self {
        Self::PermissionDenied { tool: tool.into() }
    }

    pub fn is_user_rejection(&self) -> bool {
        matches!(self, ToolError::UserRejected { .. })
    }

    pub fn user_facing_message(&self) -> String {
        match self {
            ToolError::UserRejected { .. } => "User rejected".to_string(),
            ToolError::PermissionDenied { .. } => "Permission denied".to_string(),
            _ => format!("Error: {}", self),
        }
    }

    pub fn llm_message(&self) -> String {
        match self {
            ToolError::PermissionDenied { tool } => {
                format!("Permission denied for {}", tool)
            }
            _ => format!("Error: {}", self),
        }
    }
}

// Manual From implementations since we can't use #[from] with Clone
impl From<std::io::Error> for ToolError {
    fn from(err: std::io::Error) -> Self {
        ToolError::IoError(err)
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(err: serde_json::Error) -> Self {
        ToolError::SerializationError(err)
    }
}

pub type ToolResult<T> = Result<T, ToolError>;
