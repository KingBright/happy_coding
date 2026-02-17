//! WebSocket message protocol

use super::{Artifact, FileEntry, MachineInfo, Session, SessionStatus};
use serde::{Deserialize, Serialize};

/// Client -> Server messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    // Authentication
    Authenticate {
        token: String,
    },

    // Terminal
    TerminalInput {
        session_id: String,
        data: Vec<u8>,
    },
    TerminalResize {
        session_id: String,
        cols: u16,
        rows: u16,
    },
    TerminalOutput {
        session_id: String,
        data: Vec<u8>,
    },

    // Session control
    ListSessions,
    StartSession {
        tag: String,
        profile: Option<String>,
    },
    StopSession {
        session_id: String,
    },
    AttachSession {
        session_id: String,
    },
    DetachSession {
        session_id: String,
    },
    JoinSession {
        tag: String,
    },

    // File operations
    ListFiles {
        session_id: String,
        path: String,
    },
    ReadFile {
        session_id: String,
        path: String,
    },
    WriteFile {
        session_id: String,
        path: String,
        content: Vec<u8>,
    },

    // Machine
    RegisterMachine {
        name: String,
        public_key: Vec<u8>,
    },
    UpdateMachineStatus {
        machine_id: String,
        is_online: bool,
    },

    // Heartbeat
    Ping,
}

/// Server -> Client messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    // Connection
    Authenticated {
        user_id: String,
    },
    Error {
        code: String,
        message: String,
    },

    // Terminal
    TerminalOutput {
        session_id: String,
        data: Vec<u8>,
    },
    TerminalReady {
        session_id: String,
    },
    TerminalError {
        session_id: String,
        message: String,
    },

    // Session events
    SessionsList {
        sessions: Vec<Session>,
    },
    SessionStarted {
        session: Session,
    },
    SessionStopped {
        session_id: String,
    },
    SessionUpdated {
        session: Session,
    },
    SessionStatusChanged {
        session_id: String,
        status: SessionStatus,
    },

    // File events
    FileList {
        path: String,
        entries: Vec<FileEntry>,
    },
    FileContent {
        path: String,
        content: Vec<u8>,
        content_type: Option<String>,
    },
    FileError {
        path: String,
        message: String,
    },

    // Machine events
    MachineRegistered {
        machine: MachineInfo,
    },
    MachineUpdated {
        machine: MachineInfo,
    },
    MachineList {
        machines: Vec<MachineInfo>,
    },

    // Artifact events
    ArtifactCreated {
        artifact: Artifact,
    },
    ArtifactUpdated {
        artifact: Artifact,
    },

    // Heartbeat
    Pong,
}

/// RPC request/response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
