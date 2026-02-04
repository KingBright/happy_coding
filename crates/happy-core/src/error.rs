//! Error types for Happy Coding

use thiserror::Error;

/// Main error type for Happy Coding
#[derive(Error, Debug)]
pub enum HappyError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Configuration file not found in {0}")]
    ConfigNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Build error for {platform}: {message}")]
    Build { platform: String, message: String },

    #[error("Adapter not found: {0}")]
    AdapterNotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, HappyError>;
