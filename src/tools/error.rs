use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool '{tool}' not found in registry")]
    ToolNotFound { tool: String },

    #[error("Invalid arguments for tool '{tool}': {message}")]
    InvalidArguments { tool: String, message: String },

    #[error("Tool execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    #[error("Timeout executing tool '{tool}' after {seconds} seconds")]
    Timeout { tool: String, seconds: u64 },

    #[error("Path security violation: {message}")]
    SecurityViolation { message: String },

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

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Schema validation failed for tool '{tool}': {message}")]
    SchemaValidationFailed { tool: String, message: String },
}

pub type ToolResult<T> = Result<T, ToolError>;
