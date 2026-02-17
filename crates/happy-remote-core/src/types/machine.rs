//! Machine types

use super::{Capability, Platform};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A registered machine/device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub id: String,
    pub user_id: String,
    pub name: String,
    /// X25519 public key for E2E encryption
    pub public_key: Vec<u8>,
    pub platform: Platform,
    pub last_seen: DateTime<Utc>,
    pub capabilities: Vec<Capability>,
    pub ip_address: Option<String>,
    pub hostname: Option<String>,
}

impl Machine {
    pub fn new(
        id: String,
        user_id: String,
        name: String,
        public_key: Vec<u8>,
        platform: Platform,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            public_key,
            platform,
            last_seen: Utc::now(),
            capabilities: vec![Capability::Terminal, Capability::FileSystem],
            ip_address: None,
            hostname: None,
        }
    }

    pub fn touch(&mut self) {
        self.last_seen = Utc::now();
    }

    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
}

/// Machine registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineRegistration {
    pub name: String,
    pub public_key: Vec<u8>,
    pub platform: Platform,
    pub capabilities: Vec<Capability>,
    pub hostname: Option<String>,
}

/// Machine info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub last_seen: DateTime<Utc>,
    pub is_online: bool,
    pub capabilities: Vec<Capability>,
}
