//! Artifact (file) types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// File artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub session_id: String,
    pub path: String,
    pub name: String,
    pub content_hash: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// File system entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: Option<i64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub content_type: Option<String>,
}

/// File content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: Vec<u8>,
    pub content_type: Option<String>,
    pub size: i64,
}

/// File list request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilesRequest {
    pub session_id: String,
    pub path: String,
}

/// File read request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub session_id: String,
    pub path: String,
}

/// File write request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileRequest {
    pub session_id: String,
    pub path: String,
    pub content: Vec<u8>,
}
