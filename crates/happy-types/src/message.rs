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
    TerminalHistory {
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
    DeleteSession {
        session_id: String,
    },
    AttachSession {
        session_id: String,
        tag: String,
        cwd: String,
        machine_id: Option<String>,
        machine_name: Option<String>,
    },
    DetachSession {
        session_id: String,
    },
    JoinSession {
        tag: String,
    },

    // Remote session creation (from web client)
    RequestRemoteSession {
        machine_id: String,
        cwd: Option<String>,
        args: Option<String>,
    },

    // Remote session response (from CLI daemon)
    RemoteSessionResult {
        request_id: String,
        success: bool,
        session: Option<Session>,
        error: Option<String>,
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
    ListMachines,

    // Heartbeat
    Ping,

    // Git operations (requests from web client)
    GetGitStatus {
        session_id: String,
    },
    GetGitDiff {
        session_id: String,
        path: String,
    },
    GitCommit {
        session_id: String,
        message: String,
        amend: bool,
    },

    // Git operations (responses from CLI daemon)
    GitStatusResponse {
        session_id: String,
        branch: String,
        ahead: u32,
        behind: u32,
        modified: Vec<ModifiedFile>,
        staged: Vec<ModifiedFile>,
        untracked: Vec<String>,
        conflicts: Vec<String>,
    },
    GitDiffResponse {
        session_id: String,
        path: String,
        diff: String,
    },
    GitCommitResponse {
        session_id: String,
        success: bool,
        message: String,
    },
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
    TerminalHistory {
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
    SessionDeleted {
        session_id: String,
    },
    SessionUpdated {
        session: Session,
    },
    SessionStatusChanged {
        session_id: String,
        status: SessionStatus,
    },

    // Remote session creation (server to CLI daemon)
    StartRemoteSession {
        request_id: String,
        machine_id: String,
        cwd: Option<String>,
        args: Option<String>,
    },

    // Remote session response (server to web client)
    RemoteSessionResponse {
        request_id: String,
        success: bool,
        session: Option<Session>,
        error: Option<String>,
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

    // Git events (responses to client)
    GitStatus {
        session_id: String,
        branch: String,
        ahead: u32,
        behind: u32,
        modified: Vec<ModifiedFile>,
        staged: Vec<ModifiedFile>,
        untracked: Vec<String>,
        conflicts: Vec<String>,
    },
    GitDiff {
        session_id: String,
        path: String,
        diff: String,
    },
    GitCommitResult {
        session_id: String,
        success: bool,
        message: String,
    },

    // Git requests (server to CLI daemon)
    GitStatusRequest {
        session_id: String,
        requester_id: String,
    },
    GitDiffRequest {
        session_id: String,
        path: String,
        requester_id: String,
    },
    GitCommitRequest {
        session_id: String,
        message: String,
        amend: bool,
        requester_id: String,
    },
}

/// Modified file info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedFile {
    pub path: String,
    pub change_type: ChangeType,
    pub additions: u32,
    pub deletions: u32,
}

/// Git change type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
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
