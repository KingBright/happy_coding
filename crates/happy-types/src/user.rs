//! User types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegistration {
    pub email: String,
    pub name: Option<String>,
    pub password: String,
}

/// User login request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLogin {
    pub email: String,
    pub password: String,
}

/// Authentication tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

/// Access key for API authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_prefix: String,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Access key creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKeyCreate {
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_in_days: Option<i32>,
}

/// Access key with full secret (only shown once)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKeyWithSecret {
    #[serde(flatten)]
    pub key: AccessKey,
    pub secret: String,
}
