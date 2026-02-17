//! Core domain types

pub mod artifact;
pub mod encryption;
pub mod machine;
pub mod message;
pub mod session;
pub mod user;

pub use artifact::*;
pub use encryption::*;
pub use machine::*;
pub use message::*;
pub use session::*;
pub use user::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Platform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

impl Platform {
    /// Get the current platform
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Platform::Linux; // Default to Linux for other Unix systems
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::MacOS => write!(f, "macos"),
            Platform::Linux => write!(f, "linux"),
            Platform::Windows => write!(f, "windows"),
        }
    }
}

/// AI Provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AIProvider {
    Anthropic,
    OpenAI,
    Azure,
    Gemini,
}

/// Capabilities a machine can have
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Terminal,
    FileSystem,
    Notifications,
    Voice,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::Terminal => write!(f, "terminal"),
            Capability::FileSystem => write!(f, "file_system"),
            Capability::Notifications => write!(f, "notifications"),
            Capability::Voice => write!(f, "voice"),
        }
    }
}

/// AI Backend Profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProfile {
    pub name: String,
    pub provider: AIProvider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub env_vars: HashMap<String, String>,
}

/// Settings persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub version: String,
    pub user_id: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub server_url: String,
    pub webapp_url: String,
    pub profiles: Vec<AIProfile>,
    pub active_profile: Option<String>,
    #[serde(default)]
    pub machines: Vec<Machine>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            user_id: None,
            access_token: None,
            refresh_token: None,
            server_url: "https://api.happy-remote.dev".to_string(),
            webapp_url: "https://app.happy-remote.dev".to_string(),
            profiles: Vec::new(),
            active_profile: None,
            machines: Vec::new(),
        }
    }
}
