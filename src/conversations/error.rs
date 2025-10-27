use std::fmt;

/// Represents different types of errors that can occur during tool execution
#[derive(Debug, Clone)]
pub enum ToolExecutionError {
    /// User rejected the tool execution during approval
    UserRejected { reason: String },
    /// Tool is unknown or not available
    UnknownTool { name: String },
    /// Tool arguments don't match the schema
    InvalidArguments { tool_name: String, details: String },
    /// Permission denied for the operation
    PermissionDenied { operation: String },
    /// Generic execution error
    ExecutionFailed { tool_name: String, message: String },
}

impl ToolExecutionError {
    pub fn user_rejected(reason: impl Into<String>) -> Self {
        Self::UserRejected {
            reason: reason.into(),
        }
    }

    pub fn unknown_tool(name: impl Into<String>) -> Self {
        Self::UnknownTool { name: name.into() }
    }

    pub fn invalid_arguments(tool_name: impl Into<String>, details: impl Into<String>) -> Self {
        Self::InvalidArguments {
            tool_name: tool_name.into(),
            details: details.into(),
        }
    }

    pub fn permission_denied(operation: impl Into<String>) -> Self {
        Self::PermissionDenied {
            operation: operation.into(),
        }
    }

    pub fn execution_failed(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    /// Check if this error represents a user rejection
    pub fn is_user_rejection(&self) -> bool {
        matches!(self, ToolExecutionError::UserRejected { .. })
    }

    /// Check if this error represents a permission denial
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, ToolExecutionError::PermissionDenied { .. })
    }
}

impl fmt::Display for ToolExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolExecutionError::UserRejected { reason } => {
                write!(f, "Operation rejected: {}", reason)
            }
            ToolExecutionError::UnknownTool { name } => {
                write!(f, "Unknown tool: {}", name)
            }
            ToolExecutionError::InvalidArguments { tool_name, details } => {
                write!(
                    f,
                    "Tool '{}' arguments do not match schema: {}",
                    tool_name, details
                )
            }
            ToolExecutionError::PermissionDenied { operation } => {
                write!(f, "Permission denied for {} operation", operation)
            }
            ToolExecutionError::ExecutionFailed { tool_name, message } => {
                write!(f, "Tool '{}' execution failed: {}", tool_name, message)
            }
        }
    }
}

impl std::error::Error for ToolExecutionError {}
