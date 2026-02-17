//! Error types for Happy Remote

use thiserror::Error;

pub type Result<T> = std::result::Result<T, HappyError>;

#[derive(Error, Debug)]
pub enum HappyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid secret key")]
    InvalidSecretKey,

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Machine not found: {0}")]
    MachineNotFound(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("PTY error: {0}")]
    Pty(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<serde_json::Error> for HappyError {
    fn from(e: serde_json::Error) -> Self {
        HappyError::Serialization(e.to_string())
    }
}

impl From<serde_yaml::Error> for HappyError {
    fn from(e: serde_yaml::Error) -> Self {
        HappyError::Serialization(e.to_string())
    }
}

// Note: sodiumoxide::Error removed - using stub implementation
// impl From<sodiumoxide::Error> for HappyError {
//     fn from(e: sodiumoxide::Error) -> Self {
//         HappyError::Encryption(e.to_string())
//     }
// }
