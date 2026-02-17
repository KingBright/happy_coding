use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonRequest {
    StartSession {
        id: Option<String>,
        tag: String,
        token: String,
        server_url: String,
        cwd: String,
    },
    StopSession {
        session_id: String,
    },
    ListSessions,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    Ok,
    SessionStarted { session_id: String },
    Sessions(Vec<String>),
    Error(String),
}
