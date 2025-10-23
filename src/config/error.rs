use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found at {path}")]
    NotFound { path: PathBuf },

    #[error("Invalid TOML syntax: {0}")]
    InvalidToml(#[from] toml::de::Error),

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid value for {field}: {value}")]
    InvalidValue { field: String, value: String },

    #[error("Permission denied accessing config file")]
    PermissionDenied,

    #[error("Failed to get home directory")]
    NoHomeDirectory,

    #[error("Unknown backend config key: {key}")]
    UnknownConfigKey { key: String },

    #[error("Failed to serialize config: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type ConfigResult<T> = Result<T, ConfigError>;
