//! Session types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Initializing,
    Running,
    Paused,
    Terminated,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Initializing => write!(f, "initializing"),
            SessionStatus::Running => write!(f, "running"),
            SessionStatus::Paused => write!(f, "paused"),
            SessionStatus::Terminated => write!(f, "terminated"),
        }
    }
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub claude_version: Option<String>,
    pub shell: String,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "/".to_string()),
            env: HashMap::new(),
            claude_version: None,
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
        }
    }
}

/// A remote session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub tag: String,
    pub user_id: String,
    pub machine_id: String,
    pub status: SessionStatus,
    /// Per-session encryption key (encrypted with server's public key)
    pub encrypted_data_key: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub metadata: SessionMetadata,
}

impl Session {
    pub fn new(id: String, tag: String, user_id: String, machine_id: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            tag,
            user_id,
            machine_id,
            status: SessionStatus::Initializing,
            encrypted_data_key: None,
            created_at: now,
            last_activity: now,
            metadata: SessionMetadata::default(),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Running | SessionStatus::Paused)
    }

    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}
