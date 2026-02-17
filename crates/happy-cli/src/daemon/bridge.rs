//! WebSocket-Multiplexer Bridge
//!
//! Bridges terminal I/O between SessionMultiplexer and remote WebSocket
//! used for the Remote Relay feature.

use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

// Import shared message types from happy_types
use happy_types::{ClientMessage, ServerMessage};

/// Bridge between Multiplexer and Remote WebSocket
pub struct RemoteRelayBridge {
    session_id: String,
    tag: String,
    token: String,
    ws_url: String,
    cwd: String,
    machine_id: String,
    machine_name: String,
    bridge_spawner_tx: Option<tokio::sync::mpsc::UnboundedSender<(String, String, String)>>,
}

impl RemoteRelayBridge {
    pub fn new(
        session_id: String,
        tag: String,
        token: String,
        server_url: String,
        cwd: String,
        machine_id: String,
        machine_name: String,
        bridge_spawner_tx: Option<tokio::sync::mpsc::UnboundedSender<(String, String, String)>>,
    ) -> Self {
        let ws_url = build_ws_url(&server_url);

        Self {
            session_id,
            tag,
            token,
            ws_url,
            cwd,
            machine_id,
            machine_name,
            bridge_spawner_tx,
        }
    }

    /// Start the bridge connecting Multiplexer channels to WebSocket
    pub async fn run(
        &self,
        multiplexer_input: Arc<super::multiplexer::SessionMultiplexer>,
    ) -> Result<()> {
        info!("Starting RemoteRelayBridge for session {}", self.session_id);

        let mut backoff = 1;

        loop {
            info!(
                "Attempting to connect bridge for session {}...",
                self.session_id
            );

            match self.connect_and_stream(multiplexer_input.clone()).await {
                Ok(_) => {
                    info!("Bridge connection closed normally");
                    break; // Exit if closed normally (e.g. session ended)
                }
                Err(e) => {
                    error!("Bridge connection failed: {}", e);
                    // Exponential backoff
                    error!("Retrying in {} seconds...", backoff);
                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff)).await;
                    backoff = std::cmp::min(backoff * 2, 30); // Max 30s backoff
                }
            }
        }

        info!("RemoteRelayBridge ended for session {}", self.session_id);

        // Detach from multiplexer
        multiplexer_input
            .detach_client(&self.session_id, "remote-relay")
            .await;

        Ok(())
    }

    async fn connect_and_stream(
        &self,
        multiplexer: Arc<super::multiplexer::SessionMultiplexer>,
    ) -> Result<()> {
        // 1. Attach to session to get output
        let (mut output_rx, buffer) = multiplexer
            .attach_client(&self.session_id, "remote-relay")
            .await
            .context("Failed to attach to session")?;

        // Drain any messages that might have been captured in the buffer already
        while output_rx.try_recv().is_ok() {}

        // 2. Connect WebSocket
        info!("Connecting to WebSocket: {}", self.ws_url);
        let (ws_stream, _) = connect_websocket(&self.ws_url)
            .await
            .with_context(|| format!("Failed to connect to WebSocket: {}", self.ws_url))?;
        info!("Connected to WebSocket: {}", self.ws_url);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // 3. Authenticate with WebSocket
        let auth_msg = ClientMessage::Authenticate {
            token: self.token.clone(),
        };
        ws_sender
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&auth_msg)?,
            ))
            .await?;
        info!("Sent authentication");

        // Wait for auth response (simple implementation)
        if let Some(Ok(msg)) = ws_receiver.next().await {
            let text = msg.to_text().unwrap_or("");
            match serde_json::from_str::<ServerMessage>(text) {
                Ok(ServerMessage::Error { code, message }) => {
                    anyhow::bail!("WebSocket authentication failed: {} - {}", code, message);
                }
                Ok(ServerMessage::Authenticated { user_id }) => {
                    info!("WebSocket authenticated as user: {}", user_id);
                }
                _ => {
                    warn!("Unexpected auth response: {}", text);
                }
            }
        }

        // 4. Attach to session as CLI bridge (Server side logic)
        // Use the cwd passed from CLI (user's shell PWD)
        let attach_msg = ClientMessage::AttachSession {
            session_id: self.session_id.clone(),
            tag: self.tag.clone(),
            cwd: self.cwd.clone(),
            machine_id: Some(self.machine_id.clone()),
            machine_name: Some(self.machine_name.clone()),
        };
        ws_sender
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&attach_msg)?,
            ))
            .await?;
        info!(
            "Attaching to remote session: {} ({}) with cwd: {}",
            self.tag, self.session_id, self.cwd
        );

        // Wait for server response to AttachSession
        let mut attach_confirmed = false;
        if let Some(Ok(msg)) = ws_receiver.next().await {
            let text = msg.to_text().unwrap_or("");
            info!("Server response to AttachSession: {}", text);
            match serde_json::from_str::<ServerMessage>(text) {
                Ok(ServerMessage::Error { code, message }) => {
                    error!("Server rejected AttachSession: {} - {}", code, message);
                    anyhow::bail!("Server rejected session attach: {} - {}", code, message);
                }
                Ok(ServerMessage::SessionUpdated { session }) => {
                    info!("Session attached successfully: {:?}", session.id);
                    attach_confirmed = true;
                }
                Ok(ServerMessage::TerminalReady { session_id }) => {
                    info!("Terminal ready for session: {}", session_id);
                    attach_confirmed = true;
                }
                Ok(other) => {
                    info!("Received other message after AttachSession: {:?}", other);
                    attach_confirmed = true; // Assume success if we get a message
                }
                Err(e) => {
                    warn!("Failed to parse server response: {} - {}", e, text);
                }
            }
        }

        if !attach_confirmed {
            warn!("Did not receive confirmation for AttachSession");
        }

        // Use the buffer we got during attachment
        if !buffer.is_empty() {
            let output_msg = ClientMessage::TerminalHistory {
                session_id: self.session_id.clone(),
                data: buffer,
            };
            let msg_text = serde_json::to_string(&output_msg).unwrap_or_default();
            let _ = ws_sender
                .send(tokio_tungstenite::tungstenite::Message::Text(msg_text))
                .await;
        }

        // Clone for use in the loop
        let ws_sender = Arc::new(tokio::sync::Mutex::new(ws_sender));
        let multiplexer_clone = multiplexer.clone();
        let session_id = self.session_id.clone();

        // Main bridge loop
        info!("Starting main bridge loop for session {}", session_id);
        loop {
            tokio::select! {
                // Read from Multiplexer and send to WebSocket
                result = output_rx.recv() => {
                    match result {
                        Ok(data) => {
                            // trace!("Bridge received {} bytes from PTY", data.len());
                            let output_msg = ClientMessage::TerminalOutput {
                                session_id: session_id.clone(),
                                data: data.to_vec(),
                            };
                            let msg_text = serde_json::to_string(&output_msg).unwrap_or_default();
                            let mut sender = ws_sender.lock().await;
                            if let Err(e) = sender
                                .send(tokio_tungstenite::tungstenite::Message::Text(msg_text))
                                .await
                            {
                                error!("Failed to send to WebSocket: {}", e);
                                return Err(anyhow::anyhow!("Lost connection to server"));
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Output channel closed for session {}", session_id);
                            return Ok(()); // Session ended
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Output channel lagged by {} messages for session {}", n, session_id);
                        }
                    }
                }

                // Read from WebSocket and forward to Multiplexer
                msg_opt = ws_receiver.next() => {
                    match msg_opt {
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                            // debug!("Bridge received text from server: {} bytes", text.len());
                             if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                                match server_msg {
                                    ServerMessage::TerminalOutput { session_id, data } => {
                                        // info!("Bridge received TerminalOutput from server for session {} ({} bytes)", session_id, data.len());
                                        if let Ok(client_msg) = serde_json::from_slice::<ClientMessage>(&data) {
                                             handle_client_message(client_msg, &multiplexer, &session_id, ws_sender.clone()).await;
                                        } else {
                                            // Fallback: use the session_id from TerminalOutput
                                            let _ = multiplexer.send_input(&session_id, data).await;
                                        }
                                    }
                                    ServerMessage::StartRemoteSession { request_id, machine_id, cwd, args } => {
                                        info!("Received StartRemoteSession request: request_id={}, machine_id={}, cwd={:?}", request_id, machine_id, cwd);
                                        handle_remote_session_request(
                                            request_id,
                                            machine_id,
                                            cwd,
                                            args,
                                            &multiplexer_clone,
                                            ws_sender.clone(),
                                            self.bridge_spawner_tx.clone(),
                                        ).await;
                                    }
                                    ServerMessage::Error { code, message } => {
                                        error!("Relay server error: {} - {}", code, message);
                                    }
                                    ServerMessage::SessionStopped { session_id } => {
                                        info!("Received SessionStopped for session {}. Killing local session.", session_id);
                                        let _ = multiplexer.kill_session(&session_id).await;
                                    }
                                    ServerMessage::SessionDeleted { session_id } => {
                                        info!("Received SessionDeleted for session {}. Killing local session.", session_id);
                                        let _ = multiplexer.kill_session(&session_id).await;
                                    }
                                    ServerMessage::GitStatusRequest { session_id, requester_id } => {
                                        info!("Received GitStatusRequest for session {} from {}", session_id, requester_id);
                                        handle_git_status_request(&session_id, &requester_id, &multiplexer_clone, ws_sender.clone()).await;
                                    }
                                    ServerMessage::GitDiffRequest { session_id, path, requester_id } => {
                                        info!("Received GitDiffRequest for session {} path {} from {}", session_id, path, requester_id);
                                        handle_git_diff_request(&session_id, &path, &requester_id, &multiplexer_clone, ws_sender.clone()).await;
                                    }
                                    ServerMessage::GitCommitRequest { session_id, message, amend, requester_id } => {
                                        info!("Received GitCommitRequest for session {} amend={} from {}", session_id, amend, requester_id);
                                        handle_git_commit_request(&session_id, &message, amend, &requester_id, &multiplexer_clone, ws_sender.clone()).await;
                                    }
                                    _ => {}
                                }
                            } else if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                handle_client_message(client_msg, &multiplexer, &session_id, ws_sender.clone()).await;
                            }
                        }
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(_data))) => {
                            // Binary messages should not be used for input anymore
                            warn!("Bridge received unexpected binary message from server");
                        }
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            return Err(anyhow::anyhow!("Connection closed by server"));
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            return Err(anyhow::anyhow!("WebSocket error: {}", e));
                        }
                        None => {
                            info!("WebSocket stream ended");
                            return Err(anyhow::anyhow!("WebSocket stream ended"));
                        }
                        _ => {
                            // Ignore other messages (Ping, Pong, etc.)
                        }
                    }
                }

                else => {
                    info!("Bridge loop exiting - all channels closed for session {}", session_id);
                    return Ok(());
                }
            }
        }
    }
}

/// Handle a remote session creation request from the web UI
async fn handle_remote_session_request(
    request_id: String,
    machine_id: String,
    cwd: Option<String>,
    _args: Option<String>,
    multiplexer: &Arc<super::multiplexer::SessionMultiplexer>,
    ws_sender: Arc<
        tokio::sync::Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
    bridge_spawner_tx: Option<tokio::sync::mpsc::UnboundedSender<(String, String, String)>>,
) {
    use super::multiplexer::CreateSessionRequest;
    use portable_pty::PtySize;

    // Generate a unique tag for the new session
    let tag = generate_session_tag();

    // Determine working directory - use cwd from request, not daemon's working dir
    let working_dir = cwd
        .map(|p| std::path::PathBuf::from(&p))
        .unwrap_or_else(|| {
            // If no cwd specified, use user's HOME directory, not daemon's current_dir
            dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
        });

    // Create the session
    let request = CreateSessionRequest {
        id: None,
        tag: tag.clone(),
        command: "claude".to_string(),
        working_dir: working_dir.clone(),
        env_vars: vec![],
        size: PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        },
    };

    let result = multiplexer.create_session(request).await;

    // Send response back to server
    let response_msg = match result {
        Ok(session) => {
            let session_guard = session.read().await;
            let session_id = session_guard.id.clone();
            let session_tag = session_guard.tag.clone();
            let metadata = session_guard.get_metadata().await;

            // Get machine name - prefer macOS ComputerName over hostname
            let machine_name = get_machine_name();

            let session_info = happy_types::Session {
                id: session_id.clone(),
                tag: session_tag.clone(),
                user_id: String::new(), // Will be filled by server
                machine_id,             // Use machine_id from server
                machine_name,
                status: happy_types::SessionStatus::Initializing,
                encrypted_data_key: None,
                created_at: metadata.created_at,
                last_activity: metadata.last_activity,
                metadata: happy_types::SessionMetadata {
                    cwd: metadata.working_dir.to_string_lossy().to_string(),
                    env: metadata.env_vars.into_iter().collect(),
                    claude_version: None,
                    shell: std::env::var("SHELL").unwrap_or_default(),
                },
            };
            drop(session_guard);

            info!(
                "Remote session created successfully: {} ({})",
                tag, session_id
            );

            // Request a new bridge for this session if we have a spawner
            if let Some(tx) = bridge_spawner_tx {
                if let Err(e) = tx.send((
                    session_id.clone(),
                    session_tag,
                    working_dir.to_string_lossy().to_string(),
                )) {
                    error!("Failed to request bridge spawn: {}", e);
                }
            }

            ClientMessage::RemoteSessionResult {
                request_id,
                success: true,
                session: Some(session_info),
                error: None,
            }
        }
        Err(e) => {
            error!("Failed to create remote session: {}", e);
            ClientMessage::RemoteSessionResult {
                request_id,
                success: false,
                session: None,
                error: Some(e.to_string()),
            }
        }
    };

    let mut sender = ws_sender.lock().await;
    if let Err(e) = sender
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&response_msg).unwrap_or_default(),
        ))
        .await
    {
        error!("Failed to send remote session result: {}", e);
    }
}

/// Get machine name - prefer macOS ComputerName for user-friendly name
/// Get machine name for bridge
fn get_machine_name() -> String {
    happy_core::utils::get_machine_name()
}

/// Generate a random session tag (adjective-noun-number format)
fn generate_session_tag() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let adjectives = [
        "happy", "brave", "calm", "quick", "wise", "bold", "kind", "fresh",
    ];
    let nouns = [
        "lion", "tiger", "eagle", "owl", "fox", "wolf", "bear", "deer",
    ];

    let adj = adjectives[rand::random::<usize>() % adjectives.len()];
    let noun = nouns[rand::random::<usize>() % nouns.len()];
    let num = COUNTER.fetch_add(1, Ordering::Relaxed) % 100;

    format!("{}-{}-{}", adj, noun, num)
}

fn build_ws_url(server_url: &str) -> String {
    let mut ws_url = server_url.trim_end_matches('/').to_string();
    ws_url = ws_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    if ws_url.contains("/ws") {
        return ws_url;
    }
    if let Some(idx) = ws_url.find("/api/v1") {
        return format!("{}{}", &ws_url[..idx], "/ws");
    }
    if let Some(idx) = ws_url.find("/api") {
        return format!("{}{}", &ws_url[..idx], "/ws");
    }
    format!("{}/ws", ws_url)
}

async fn connect_websocket(
    ws_url: &str,
) -> Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::handshake::client::Response,
)> {
    tokio_tungstenite::connect_async(ws_url)
        .await
        .context("WebSocket connection failed")
}

async fn handle_client_message(
    msg: ClientMessage,
    multiplexer: &Arc<super::multiplexer::SessionMultiplexer>,
    _session_id: &str,
    _ws_sender: Arc<
        tokio::sync::Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
) {
    match msg {
        ClientMessage::TerminalInput { session_id, data } => {
            info!(
                "Bridge forwarding {} bytes of input to session {}",
                data.len(),
                session_id
            );
            if let Err(e) = multiplexer.send_input(&session_id, data).await {
                error!("Failed to send input to session {}: {}", session_id, e);
            }
        }
        ClientMessage::TerminalResize {
            session_id,
            cols,
            rows,
        } => {
            info!(
                "Bridge resizing session {} to {}x{}",
                session_id, cols, rows
            );
            if let Err(e) = multiplexer.resize_session(&session_id, cols, rows).await {
                error!("Failed to resize session {}: {}", session_id, e);
            }
        }
        _ => {}
    }
}

/// Handle git status request
async fn handle_git_status_request(
    session_id: &str,
    requester_id: &str,
    multiplexer: &Arc<super::multiplexer::SessionMultiplexer>,
    ws_sender: Arc<
        tokio::sync::Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
) {
    // Get session working directory
    let cwd = match multiplexer.get_session_cwd(session_id).await {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get session {} cwd: {}", session_id, e);
            return;
        }
    };

    let status = match git_operations::get_git_status(&cwd).await {
        Ok(status) => status,
        Err(e) => {
            error!("Failed to get git status: {}", e);
            // Send error response and return early
            let error_response = ServerMessage::Error {
                code: "git_status_failed".to_string(),
                message: format!("Failed to get git status: {}", e),
            };
            let mut sender = ws_sender.lock().await;
            if let Err(e) = sender
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    serde_json::to_string(&error_response).unwrap_or_default(),
                ))
                .await
            {
                error!("Failed to send git status error response: {}", e);
            }
            return;
        }
    };

    let response = ClientMessage::GitStatusResponse {
        session_id: session_id.to_string(),
        branch: status.branch,
        ahead: status.ahead,
        behind: status.behind,
        modified: status.modified,
        staged: status.staged,
        untracked: status.untracked,
        conflicts: status.conflicts,
    };

    let mut sender = ws_sender.lock().await;
    if let Err(e) = sender
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&response).unwrap_or_default(),
        ))
        .await
    {
        error!("Failed to send git status response: {}", e);
    }
}

/// Handle git diff request
async fn handle_git_diff_request(
    session_id: &str,
    path: &str,
    _requester_id: &str,
    multiplexer: &Arc<super::multiplexer::SessionMultiplexer>,
    ws_sender: Arc<
        tokio::sync::Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
) {
    let cwd = match multiplexer.get_session_cwd(session_id).await {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get session {} cwd: {}", session_id, e);
            return;
        }
    };

    let diff = match git_operations::get_git_diff(&cwd, path).await {
        Ok(diff) => diff,
        Err(e) => {
            error!("Failed to get git diff: {}", e);
            format!("Error: {}", e)
        }
    };

    let response = ClientMessage::GitDiffResponse {
        session_id: session_id.to_string(),
        path: path.to_string(),
        diff,
    };

    let mut sender = ws_sender.lock().await;
    if let Err(e) = sender
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&response).unwrap_or_default(),
        ))
        .await
    {
        error!("Failed to send git diff response: {}", e);
    }
}

/// Handle git commit request
async fn handle_git_commit_request(
    session_id: &str,
    message: &str,
    amend: bool,
    _requester_id: &str,
    multiplexer: &Arc<super::multiplexer::SessionMultiplexer>,
    ws_sender: Arc<
        tokio::sync::Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
) {
    let cwd = match multiplexer.get_session_cwd(session_id).await {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get session {} cwd: {}", session_id, e);
            return;
        }
    };

    let (success, msg) = match git_operations::git_commit(&cwd, message, amend).await {
        Ok(output) => (true, output),
        Err(e) => (false, format!("Commit failed: {}", e)),
    };

    let response = ClientMessage::GitCommitResponse {
        session_id: session_id.to_string(),
        success,
        message: msg,
    };

    let mut sender = ws_sender.lock().await;
    if let Err(e) = sender
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&response).unwrap_or_default(),
        ))
        .await
    {
        error!("Failed to send git commit result: {}", e);
    }
}

/// Git operations module
mod git_operations {
    use std::path::Path;
    use tokio::process::Command;
    use tracing::{debug, error};

    /// Git status information
    #[derive(Debug)]
    pub struct GitStatus {
        pub branch: String,
        pub ahead: u32,
        pub behind: u32,
        pub modified: Vec<happy_types::ModifiedFile>,
        pub staged: Vec<happy_types::ModifiedFile>,
        pub untracked: Vec<String>,
        pub conflicts: Vec<String>,
    }

    /// Get git status for a directory
    pub async fn get_git_status(cwd: &Path) -> anyhow::Result<GitStatus> {
        // Get branch info
        let branch_output = Command::new("git")
            .args(["-C", cwd.to_str().unwrap_or("."), "status", "--porcelain", "-b"])
            .output()
            .await?;

        if !branch_output.status.success() {
            anyhow::bail!("Not a git repository");
        }

        let output_str = String::from_utf8_lossy(&branch_output.stdout);
        let lines: Vec<&str> = output_str.lines().collect();

        // Parse branch info from first line
        let (branch, ahead, behind) = if let Some(first_line) = lines.first() {
            parse_branch_line(first_line)
        } else {
            ("unknown".to_string(), 0, 0)
        };

        // Parse file statuses
        let mut modified = Vec::new();
        let mut staged = Vec::new();
        let mut untracked = Vec::new();
        let mut conflicts = Vec::new();

        for line in lines.iter().skip(1) {
            if line.len() < 3 {
                continue;
            }

            let index_status = line.chars().next().unwrap_or(' ');
            let worktree_status = line.chars().nth(1).unwrap_or(' ');
            let file_path = &line[3..];

            match (index_status, worktree_status) {
                ('M', ' ') | ('A', ' ') | ('D', ' ') | ('R', ' ') => {
                    // Staged changes
                    staged.push(happy_types::ModifiedFile {
                        path: file_path.to_string(),
                        change_type: map_change_type(index_status),
                        additions: 0,
                        deletions: 0,
                    });
                }
                ('M', 'M') | ('A', 'M') | ('D', 'M') => {
                    // Both staged and unstaged
                    staged.push(happy_types::ModifiedFile {
                        path: file_path.to_string(),
                        change_type: map_change_type(index_status),
                        additions: 0,
                        deletions: 0,
                    });
                    modified.push(happy_types::ModifiedFile {
                        path: file_path.to_string(),
                        change_type: happy_types::ChangeType::Modified,
                        additions: 0,
                        deletions: 0,
                    });
                }
                (' ', 'M') | (' ', 'D') => {
                    // Unstaged changes
                    modified.push(happy_types::ModifiedFile {
                        path: file_path.to_string(),
                        change_type: happy_types::ChangeType::Modified,
                        additions: 0,
                        deletions: 0,
                    });
                }
                ('?', '?') => {
                    untracked.push(file_path.to_string());
                }
                ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => {
                    conflicts.push(file_path.to_string());
                }
                _ => {
                    debug!("Unknown git status: {} {}", index_status, worktree_status);
                }
            }
        }

        Ok(GitStatus {
            branch,
            ahead,
            behind,
            modified,
            staged,
            untracked,
            conflicts,
        })
    }

    fn parse_branch_line(line: &str) -> (String, u32, u32) {
        // Format: ## branch-name...upstream [ahead N, behind M] or ## branch-name
        let line = line.trim_start_matches("## ");

        if let Some(idx) = line.find("...") {
            let branch = &line[..idx];
            let rest = &line[idx + 3..];

            let ahead = rest.find("ahead ").and_then(|i| {
                rest[i + 6..].split(',').next()?.split(' ').next()?.parse().ok()
            }).unwrap_or(0);

            let behind = rest.find("behind ").and_then(|i| {
                rest[i + 7..].split(',').next()?.split(' ').next()?.parse().ok()
            }).unwrap_or(0);

            (branch.to_string(), ahead, behind)
        } else {
            (line.split_whitespace().next().unwrap_or("unknown").to_string(), 0, 0)
        }
    }

    fn map_change_type(status: char) -> happy_types::ChangeType {
        match status {
            'A' => happy_types::ChangeType::Added,
            'M' => happy_types::ChangeType::Modified,
            'D' => happy_types::ChangeType::Deleted,
            'R' => happy_types::ChangeType::Renamed,
            _ => happy_types::ChangeType::Modified,
        }
    }

    /// Get git diff for a specific file
    pub async fn get_git_diff(cwd: &Path, path: &str) -> anyhow::Result<String> {
        let output = Command::new("git")
            .args(["-C", cwd.to_str().unwrap_or("."), "diff", path])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git diff failed: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute git commit
    pub async fn git_commit(cwd: &Path, message: &str, amend: bool) -> anyhow::Result<String> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(cwd);
        cmd.arg("commit");

        if amend {
            cmd.arg("--amend");
            if !message.is_empty() {
                cmd.arg("-m").arg(message);
            } else {
                cmd.arg("--no-edit");
            }
        } else {
            cmd.arg("-m").arg(message);
        }

        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("{}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
