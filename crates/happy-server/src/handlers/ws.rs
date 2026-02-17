//! WebSocket handler for real-time terminal communication
//!
//! Routes messages between:
//! - CLI daemon (PTY bridge) - sends TerminalOutput, receives TerminalInput
//! - Web clients - sends TerminalInput, receives TerminalOutput

use crate::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{sink::SinkExt, stream::StreamExt};
use happy_types::{ClientMessage, ServerMessage, SessionStatus};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Connection manager for routing messages between CLI and web clients
#[derive(Clone)]
pub struct ConnectionManager {
    /// Maps session_id to the CLI bridge connection (daemon)
    cli_connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<ServerMessage>>>>,
    /// Maps session_id to list of web client connections
    web_connections: Arc<RwLock<HashMap<String, Vec<mpsc::UnboundedSender<ServerMessage>>>>>,
    /// Maps machine_id to the CLI daemon connection (for remote session creation)
    /// Maps machine_id to map of connection_id -> CLI daemon connection
    machine_connections:
        Arc<RwLock<HashMap<String, HashMap<String, mpsc::UnboundedSender<ServerMessage>>>>>,
    /// Maps request_id to web client connection (for remote session responses)
    pending_requests: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<ServerMessage>>>>,
    output_buffers: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// All authenticated user connections (for broadcasting global updates like MachineList)
    user_connections: Arc<RwLock<Vec<(String, mpsc::UnboundedSender<ServerMessage>)>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            cli_connections: Arc::new(RwLock::new(HashMap::new())),
            web_connections: Arc::new(RwLock::new(HashMap::new())),
            machine_connections: Arc::new(RwLock::new(HashMap::new())),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            output_buffers: Arc::new(RwLock::new(HashMap::new())),
            user_connections: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a user connection for global broadcasts
    pub async fn register_user(&self, user_id: &str, tx: mpsc::UnboundedSender<ServerMessage>) {
        let mut conns = self.user_connections.write().await;
        conns.push((user_id.to_string(), tx));
        info!("User registered for global broadcasts: {}", user_id);
    }

    /// Unregister a user connection
    pub async fn unregister_user(&self, user_id: &str) {
        let mut conns = self.user_connections.write().await;
        conns.retain(|(id, _)| id != user_id);
        info!("User unregistered from global broadcasts: {}", user_id);
    }

    /// Broadcast a message to all connected users (for global updates like MachineList, SessionDeleted)
    pub async fn broadcast_to_all_users(&self, msg: ServerMessage) {
        let conns = self.user_connections.read().await;
        for (user_id, tx) in conns.iter() {
            if let Err(_) = tx.send(msg.clone()) {
                tracing::debug!("Failed to send to user {}", user_id);
            }
        }
    }

    /// Register CLI bridge connection for a session
    pub async fn register_cli(&self, session_id: &str, tx: mpsc::UnboundedSender<ServerMessage>) {
        let mut conns = self.cli_connections.write().await;
        conns.insert(session_id.to_string(), tx);
        info!("CLI bridge registered for session {}", session_id);
    }

    /// Unregister CLI bridge connection
    pub async fn unregister_cli(&self, session_id: &str) {
        let mut conns = self.cli_connections.write().await;
        conns.remove(session_id);
        info!("CLI bridge unregistered for session {}", session_id);
    }

    /// Register machine connection (for remote session creation)
    pub async fn register_machine(
        &self,
        machine_id: &str,
        connection_id: &str,
        tx: mpsc::UnboundedSender<ServerMessage>,
    ) {
        let mut conns = self.machine_connections.write().await;
        conns
            .entry(machine_id.to_string())
            .or_default()
            .insert(connection_id.to_string(), tx);
        info!(
            "Machine daemon connection registered: {} (conn_id={})",
            machine_id, connection_id
        );
    }

    /// Unregister machine connection
    pub async fn unregister_machine(&self, machine_id: &str, connection_id: &str) {
        let mut conns = self.machine_connections.write().await;
        if let Some(machine_conns) = conns.get_mut(machine_id) {
            machine_conns.remove(connection_id);
            if machine_conns.is_empty() {
                conns.remove(machine_id);
                info!(
                    "Machine daemon offline (all connections lost): {}",
                    machine_id
                );
            } else {
                info!(
                    "Machine daemon connection removed: {} (conn_id={}). Remaining connections: {}",
                    machine_id,
                    connection_id,
                    machine_conns.len()
                );
            }
        }
    }

    /// Check if a machine has an active daemon connection
    pub async fn has_machine(&self, machine_id: &str) -> bool {
        let conns = self.machine_connections.read().await;
        conns.contains_key(machine_id) && !conns.get(machine_id).unwrap().is_empty()
    }

    /// Get machine connection for sending remote session requests
    pub async fn get_machine_tx(
        &self,
        machine_id: &str,
    ) -> Option<mpsc::UnboundedSender<ServerMessage>> {
        let conns = self.machine_connections.read().await;
        // Return any active connection for this machine
        conns
            .get(machine_id)
            .and_then(|map| map.values().next().cloned())
    }

    /// Register pending request (for tracking remote session creation)
    pub async fn register_pending_request(
        &self,
        request_id: &str,
        tx: mpsc::UnboundedSender<ServerMessage>,
    ) {
        let mut reqs = self.pending_requests.write().await;
        reqs.insert(request_id.to_string(), tx);
    }

    /// Get and remove pending request
    pub async fn take_pending_request(
        &self,
        request_id: &str,
    ) -> Option<mpsc::UnboundedSender<ServerMessage>> {
        let mut reqs = self.pending_requests.write().await;
        reqs.remove(request_id)
    }

    /// Register web client connection for a session
    pub async fn register_web(&self, session_id: &str, tx: mpsc::UnboundedSender<ServerMessage>) {
        let mut conns = self.web_connections.write().await;
        conns
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
        info!("Web client registered for session {}", session_id);
    }

    /// Unregister web client connection
    pub async fn unregister_web(&self, session_id: &str) {
        let mut conns = self.web_connections.write().await;
        conns.remove(session_id);
        info!("Web clients cleared for session {}", session_id);
    }

    /// Forward TerminalInput from web client to CLI bridge
    pub async fn forward_to_cli(&self, session_id: &str, msg: ServerMessage) {
        let conns = self.cli_connections.read().await;
        if let Some(tx) = conns.get(session_id) {
            let _ = tx.send(msg);
        }
    }

    /// Broadcast TerminalOutput from CLI bridge to web clients
    pub async fn broadcast_to_web(&self, session_id: &str, msg: ServerMessage) {
        let conns = self.web_connections.read().await;
        if let Some(clients) = conns.get(session_id) {
            for tx in clients {
                let _ = tx.send(msg.clone());
            }
        }
    }

    pub async fn append_output(&self, session_id: &str, data: &[u8]) {
        const MAX_BUFFER_BYTES: usize = 64 * 1024;
        let mut buffers = self.output_buffers.write().await;
        let entry = buffers.entry(session_id.to_string()).or_default();
        entry.extend_from_slice(data);
        if entry.len() > MAX_BUFFER_BYTES {
            let excess = entry.len() - MAX_BUFFER_BYTES;
            entry.drain(0..excess);
        }
    }

    pub async fn set_output_buffer(&self, session_id: &str, data: Vec<u8>) {
        const MAX_BUFFER_BYTES: usize = 64 * 1024;
        let mut buffers = self.output_buffers.write().await;
        // Truncate if too large
        let data = if data.len() > MAX_BUFFER_BYTES {
            let excess = data.len() - MAX_BUFFER_BYTES;
            data[excess..].to_vec()
        } else {
            data
        };
        buffers.insert(session_id.to_string(), data);
    }

    pub async fn get_output_buffer(&self, session_id: &str) -> Option<Vec<u8>> {
        let buffers = self.output_buffers.read().await;
        buffers.get(session_id).cloned()
    }

    /// Check if a session has an active CLI bridge
    pub async fn has_cli(&self, session_id: &str) -> bool {
        let conns = self.cli_connections.read().await;
        conns.contains_key(session_id)
    }

    /// Broadcast a message to all connected web clients across all sessions (legacy, use broadcast_to_all_users for global updates)
    pub async fn broadcast_to_all_web(&self, msg: ServerMessage) {
        let conns = self.web_connections.read().await;
        for (session_id, clients) in conns.iter() {
            for tx in clients {
                if let Err(_) = tx.send(msg.clone()) {
                    tracing::debug!("Failed to send to client in session {}", session_id);
                }
            }
        }
    }

    /// Send a message to all connections of a specific user
    pub async fn send_to_user(&self, user_id: &str, msg: ServerMessage) {
        let conns = self.user_connections.read().await;
        for (id, tx) in conns.iter() {
            if id == user_id {
                if let Err(_) = tx.send(msg.clone()) {
                    tracing::debug!("Failed to send to user {}", user_id);
                }
            }
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Client connection state
struct ClientState {
    user_id: Option<String>,
    session_id: Option<String>,
    session_ids: HashSet<String>,
    /// True if this is a CLI bridge connection (daemon)
    is_cli_bridge: bool,
    /// Connection ID (UUID)
    connection_id: String,
    /// Machine ID for daemon connections
    machine_id: Option<String>,
    /// Machine name for daemon connections
    machine_name: Option<String>,
}

/// Handle WebSocket upgrade
pub async fn handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection");

    let (mut sender, mut receiver) = socket.split();
    let mut client_state = ClientState {
        user_id: None,
        session_id: None,
        session_ids: HashSet::new(),
        is_cli_bridge: false,
        connection_id: Uuid::new_v4().to_string(),
        machine_id: None,
        machine_name: None,
    };

    // Create channel for sending messages to this client
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Spawn task to forward messages from channel to WebSocket
    let _forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received text message: {}", text);
                // Log the first 100 chars of message for debugging input issues
                let preview = if text.len() > 100 {
                    &text[0..100]
                } else {
                    &text
                };
                info!("WS Input: {}", preview);

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        let should_continue =
                            handle_message(client_msg, &state, &mut client_state, &tx).await;
                        if !should_continue {
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse message: {}", e);
                        let error = ServerMessage::Error {
                            code: "invalid_message".to_string(),
                            message: format!("Failed to parse message: {}", e),
                        };
                        let _ = tx.send(error);
                    }
                }
            }
            Ok(Message::Binary(bin)) => {
                debug!("Received binary message: {} bytes", bin.len());
                // TODO: Handle encrypted binary messages
            }
            Ok(Message::Ping(_)) => {
                // Axum handles pings automatically
            }
            Ok(Message::Pong(_)) => {
                // Axum handles pongs automatically
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }

    // Verify client_state.session_id is some before logic
    if client_state.is_cli_bridge {
        if let Some(session_id) = &client_state.session_id {
            state.conn_manager.unregister_cli(session_id).await;

            // Mark session as Terminated when CLI bridge disconnects (but keep it in database)
            // This allows the session list to show historical sessions
            if let Err(e) = state
                .session_manager
                .update_session_status(session_id, SessionStatus::Terminated)
                .await
            {
                error!("Failed to terminate session {}: {}", session_id, e);
            } else {
                info!(
                    "Session {} marked as terminated due to CLI disconnect",
                    session_id
                );
            }
            // Broadcast update to web clients so dashboard refreshes live
            let _ = state
                .conn_manager
                .broadcast_to_web(
                    session_id,
                    ServerMessage::SessionStopped {
                        session_id: session_id.clone(),
                    },
                )
                .await;
            info!("Client disconnected from session: {}", session_id);
        }
        // Unregister machine connection
        if let Some(machine_id) = &client_state.machine_id {
            state
                .conn_manager
                .unregister_machine(machine_id, &client_state.connection_id)
                .await;

            // Broadcast updated machine list to all clients
            if let Some(user_id) = &client_state.user_id {
                broadcast_machine_list(&state, user_id).await;
            }
        }
    } else {
        for session_id in client_state.session_ids {
            state.conn_manager.unregister_web(&session_id).await;
            info!("Client disconnected from session: {}", session_id);
        }
    }

    // Unregister user connection
    if let Some(user_id) = &client_state.user_id {
        state.conn_manager.unregister_user(user_id).await;
    }

    // Do NOT abort the forward_task immediately.
    // We want to allow any pending messages (like Auth Failed errors) to be flushed to the socket
    // before the task is cancelled. The task will naturally exit when `tx` is dropped (end of this function)
    // because `rx.recv()` will return `None`.
    // forward_task.abort();

    info!("WebSocket connection ended");
}

/// Handle a client message
/// Returns true to continue, false to disconnect
async fn handle_message(
    msg: ClientMessage,
    state: &AppState,
    client_state: &mut ClientState,
    tx: &mpsc::UnboundedSender<ServerMessage>,
) -> bool {
    match msg {
        ClientMessage::Ping => {
            let _ = tx.send(ServerMessage::Pong);
        }
        ClientMessage::Authenticate { token } => {
            // Validate JWT token
            match state.auth_service.validate_token(&token).await {
                Ok(user_id) => {
                    client_state.user_id = Some(user_id.clone());
                    info!("User authenticated: {}", user_id);
                    // Register this connection for global broadcasts
                    state.conn_manager.register_user(&user_id, tx.clone()).await;
                    let _ = tx.send(ServerMessage::Authenticated {
                        user_id: user_id.clone(),
                    });

                    // Send current machine list immediately so frontend knows which machines are online
                    send_machine_list_to_user(state, &user_id, tx.clone()).await;
                }
                Err(e) => {
                    warn!("Authentication failed: {}", e);
                    let _ = tx.send(ServerMessage::Error {
                        code: "auth_failed".to_string(),
                        message: "Invalid or expired token".to_string(),
                    });
                    return false; // Disconnect on auth failure
                }
            }
        }
        ClientMessage::JoinSession { tag } => {
            info!(
                "JoinSession request: tag={}, user_id={:?}",
                tag, client_state.user_id
            );
            if let Some(user_id) = &client_state.user_id {
                // Find session by tag
                match state
                    .session_manager
                    .find_session_by_tag(user_id, &tag)
                    .await
                {
                    Ok(Some(session)) => {
                        let session_id = session.id.clone();
                        info!(
                            "Found session for tag '{}': id={}, status={:?}",
                            tag, session_id, session.status
                        );
                        let is_new = client_state.session_ids.insert(session_id.clone());
                        if is_new {
                            state
                                .conn_manager
                                .register_web(&session_id, tx.clone())
                                .await;
                            info!("Registered web client for session {}", session_id);
                        } else {
                            info!("Web client already registered for session {}", session_id);
                        }

                        info!("User {} joined session {}", user_id, session_id);
                        let _ = tx.send(ServerMessage::SessionUpdated {
                            session: session.clone(),
                        });
                        let _ = tx.send(ServerMessage::TerminalReady {
                            session_id: session_id.clone(),
                        });
                        if let Some(buffer) =
                            state.conn_manager.get_output_buffer(&session.id).await
                        {
                            info!(
                                "Sending buffered output for session {}: {} bytes",
                                session.id,
                                buffer.len()
                            );
                            if !buffer.is_empty() {
                                let _ = tx.send(ServerMessage::TerminalOutput {
                                    session_id: session.id.clone(),
                                    data: buffer,
                                });
                            }
                        }
                    }
                    Ok(None) => {
                        warn!("Session not found for tag '{}' and user {}", tag, user_id);
                        let _ = tx.send(ServerMessage::Error {
                            code: "session_not_found".to_string(),
                            message: format!("Session '{}' not found", tag),
                        });
                    }
                    Err(e) => {
                        error!("Failed to find session: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "find_failed".to_string(),
                            message: "Failed to find session".to_string(),
                        });
                    }
                }
            } else {
                warn!("JoinSession rejected: not authenticated");
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }
        ClientMessage::AttachSession {
            session_id,
            tag,
            cwd,
            machine_id,
            machine_name,
        } => {
            info!("AttachSession request: session_id={}, tag={}, cwd={}, machine_id={:?}, machine_name={:?}, user_id={:?}",
                session_id, tag, cwd, machine_id, machine_name, client_state.user_id);
            // CLI daemon uses AttachSession to register as the bridge
            if let Some(user_id) = &client_state.user_id {
                // Check if session exists
                let session = match state.session_manager.get_session(&session_id).await {
                    Ok(Some(session)) if session.user_id == *user_id => {
                        // Session exists and belongs to user

                        // 1. Update machine_id if provided and different
                        let session = if let Some(ref remote_machine_id) = machine_id {
                            let name = machine_name
                                .clone()
                                .unwrap_or_else(|| "Unknown Machine".to_string());

                            // Sync machine with registry
                            if let Err(e) = state
                                .machine_registry
                                .register_machine(
                                    user_id,
                                    remote_machine_id,
                                    &name,
                                    happy_core::Platform::current(),
                                )
                                .await
                            {
                                error!("Failed to register machine in registry: {}", e);
                            } else {
                                // Broadcast updated machine list to web clients
                                broadcast_machine_list(&state, user_id).await;
                            }

                            if session.machine_id != *remote_machine_id {
                                info!(
                                    "Updating session {} machine from '{}' to '{}'",
                                    session_id, session.machine_id, remote_machine_id
                                );

                                match state
                                    .session_manager
                                    .update_session_machine(&session_id, remote_machine_id, &name)
                                    .await
                                {
                                    Ok(_) => {
                                        // Return updated session object
                                        match state.session_manager.get_session(&session_id).await {
                                            Ok(Some(updated)) => Some(updated),
                                            _ => Some(session),
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to update session machine: {}", e);
                                        Some(session)
                                    }
                                }
                            } else {
                                Some(session)
                            }
                        } else {
                            Some(session)
                        };

                        // 2. Check current status. If Terminated, reject attachment and tell CLI to cleanup
                        if let Some(ref session) = session {
                            if session.status == happy_types::SessionStatus::Terminated {
                                warn!(
                                    "AttachSession rejected: session {} is already terminated",
                                    session_id
                                );
                                let _ = tx.send(ServerMessage::SessionStopped {
                                    session_id: session_id.clone(),
                                });
                                return false;
                            }
                        }

                        // 3. Update CWD if different (only if session is still valid)
                        if let Some(session) = session {
                            if session.metadata.cwd != cwd {
                                info!(
                                    "Updating session {} cwd from '{}' to '{}'",
                                    session_id, session.metadata.cwd, cwd
                                );
                                match state
                                    .session_manager
                                    .update_session_cwd(&session_id, &cwd)
                                    .await
                                {
                                    Ok(_) => {
                                        match state.session_manager.get_session(&session_id).await {
                                            Ok(Some(updated)) => Some(updated),
                                            _ => Some(session),
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to update session cwd: {}", e);
                                        Some(session)
                                    }
                                }
                            } else {
                                Some(session)
                            }
                        } else {
                            None
                        }
                    }
                    Ok(Some(session)) => {
                        warn!(
                            "AttachSession rejected: session belongs to {}, but requester is {}",
                            session.user_id, user_id
                        );
                        let _ = tx.send(ServerMessage::Error {
                            code: "access_denied".to_string(),
                            message: "Session belongs to another user".to_string(),
                        });
                        None
                    }
                    Ok(None) => {
                        // Session not found - we used to implicitly create it here, which caused 'phantom sessions'.
                        // Now we reject it and tell the CLI to cleanup.
                        // Note: Fresh sessions from 'happy run' should already be in the DB via the API call.
                        info!(
                            "Session {} not found in DB during AttachSession. Rejecting and notifying CLI to cleanup.",
                            session_id
                        );
                        let _ = tx.send(ServerMessage::SessionDeleted {
                            session_id: session_id.clone(),
                        });
                        None
                    }
                    Err(e) => {
                        error!("AttachSession error: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "error".to_string(),
                            message: format!("Error: {}", e),
                        });
                        None
                    }
                };

                // If we have a session (existing or newly created), proceed with attachment
                if let Some(session) = session {
                    client_state.session_id = Some(session_id.clone());
                    client_state.is_cli_bridge = true;
                    client_state.machine_id = Some(session.machine_id.clone());
                    client_state.machine_name = Some(session.machine_name.clone());

                    // Register as CLI bridge
                    state
                        .conn_manager
                        .register_cli(&session_id, tx.clone())
                        .await;

                    // Register machine connection for remote session creation
                    state
                        .conn_manager
                        .register_machine(
                            &session.machine_id,
                            &client_state.connection_id,
                            tx.clone(),
                        )
                        .await;

                    // Broadcast updated machine list to all clients
                    broadcast_machine_list(state, user_id).await;

                    // Update session status to Running
                    let _ = state
                        .session_manager
                        .update_session_status(&session_id, SessionStatus::Running)
                        .await;

                    info!(
                        "CLI bridge attached to session {} (tag={}, machine={})",
                        session_id, session.tag, session.machine_name
                    );
                    let _ = tx.send(ServerMessage::SessionUpdated { session });
                    let _ = tx.send(ServerMessage::TerminalReady { session_id });
                }
            } else {
                warn!("AttachSession rejected: not authenticated");
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }
        ClientMessage::TerminalInput { session_id, data } => {
            info!(
                "Received TerminalInput for session {} ({} bytes), client_sessions: {:?}",
                session_id,
                data.len(),
                client_state.session_ids
            );
            // Verify: 1) client has joined this session, 2) CLI bridge is connected
            if client_state.session_ids.contains(&session_id) {
                // Verify CLI bridge is connected before forwarding
                if !state.conn_manager.has_cli(&session_id).await {
                    warn!(
                        "CLI bridge not connected for session {}, dropping input",
                        session_id
                    );
                    let _ = tx.send(ServerMessage::Error {
                        code: "bridge_not_connected".to_string(),
                        message: "No CLI bridge connected for this session".to_string(),
                    });
                    return true; // Continue but don't forward
                }

                // Forward to CLI bridge
                let conns = state.conn_manager.cli_connections.read().await;
                if let Some(cli_tx) = conns.get(&session_id) {
                    info!("Forwarding input to CLI bridge for session {}", session_id);
                    // Serialize the ClientMessage and send as a special wrapper
                    // CLI bridge will parse the JSON and handle it
                    let forward_msg = ClientMessage::TerminalInput {
                        session_id: session_id.clone(),
                        data: data.clone(),
                    };
                    if let Ok(json) = serde_json::to_string(&forward_msg) {
                        // Send as a custom ServerMessage that CLI can parse
                        let _ = cli_tx.send(ServerMessage::TerminalOutput {
                            session_id: session_id.clone(),
                            data: json.into_bytes(),
                        });
                    }
                } else {
                    warn!("No CLI bridge connected for session {}", session_id);
                    let _ = tx.send(ServerMessage::Error {
                        code: "no_cli".to_string(),
                        message: "No CLI bridge connected".to_string(),
                    });
                }
            } else {
                warn!(
                    "Session ID mismatch for input. Client sessions: {:?}, Message session: {}",
                    client_state.session_ids, session_id
                );
                let _ = tx.send(ServerMessage::Error {
                    code: "session_mismatch".to_string(),
                    message: format!("You are not connected to session {}", session_id),
                });
            }
        }
        ClientMessage::TerminalHistory { session_id, data } => {
            // CLI bridge sends history (initial state) -> replace buffer and broadcast
            info!(
                "TerminalHistory received: session_id={}, is_cli_bridge={}, data_len={}",
                session_id,
                client_state.is_cli_bridge,
                data.len()
            );
            if client_state.is_cli_bridge {
                if client_state.session_id.as_ref() == Some(&session_id) {
                    // Replace buffer
                    state
                        .conn_manager
                        .set_output_buffer(&session_id, data.clone())
                        .await;

                    // Broadcast history to web clients
                    let conns = state.conn_manager.web_connections.read().await;
                    info!(
                        "Broadcasting history to web clients for session {}: {} clients connected",
                        session_id,
                        conns.get(&session_id).map(|c| c.len()).unwrap_or(0)
                    );
                    if let Some(clients) = conns.get(&session_id) {
                        let msg = ServerMessage::TerminalHistory {
                            session_id: session_id.clone(),
                            data: data.clone(),
                        };
                        for tx in clients {
                            let _ = tx.send(msg.clone());
                        }
                    }
                } else {
                    error!("TerminalHistory rejected: session_id mismatch");
                }
            }
        }
        ClientMessage::TerminalOutput { session_id, data } => {
            // CLI bridge sends output -> broadcast to web clients
            info!(
                "TerminalOutput received: session_id={}, is_cli_bridge={}, expected_session_id={:?}, data_len={}",
                session_id, client_state.is_cli_bridge, client_state.session_id, data.len()
            );
            // Strict validation: must be CLI bridge AND session_id matches
            if client_state.is_cli_bridge {
                if client_state.session_id.as_ref() == Some(&session_id) {
                    state.conn_manager.append_output(&session_id, &data).await;
                    let conns = state.conn_manager.web_connections.read().await;
                    info!(
                        "Broadcasting to web clients for session {}: {} clients connected",
                        session_id,
                        conns.get(&session_id).map(|c| c.len()).unwrap_or(0)
                    );
                    if let Some(clients) = conns.get(&session_id) {
                        let msg = ServerMessage::TerminalOutput {
                            session_id: session_id.clone(),
                            data: data.clone(),
                        };
                        for tx in clients {
                            let _ = tx.send(msg.clone());
                        }
                        info!("Sent TerminalOutput to {} web clients", clients.len());
                    }
                } else {
                    error!(
                        "TerminalOutput rejected: session_id mismatch. CLI bridge expected={}, got={}",
                        client_state.session_id.as_ref().map(|s| s.as_str()).unwrap_or("none"),
                        session_id
                    );
                    let _ = tx.send(ServerMessage::Error {
                        code: "session_mismatch".to_string(),
                        message: format!(
                            "Session ID mismatch: expected {:?}, got {}",
                            client_state.session_id, session_id
                        ),
                    });
                }
            } else {
                // Non-CLI bridge clients should not send TerminalOutput
                warn!(
                    "TerminalOutput from non-CLI bridge client rejected: session_id={}, is_cli_bridge={}",
                    session_id, client_state.is_cli_bridge
                );
            }
        }
        ClientMessage::TerminalResize {
            session_id,
            cols,
            rows,
        } => {
            debug!(
                "Terminal resize for session {}: {}x{}",
                session_id, cols, rows
            );
            // Forward to CLI bridge
            if client_state.session_ids.contains(&session_id) {
                let conns = state.conn_manager.cli_connections.read().await;
                if let Some(_cli_tx) = conns.get(&session_id) {
                    // Send resize to CLI - we'd need a message type for this
                    // For now, just log it
                    debug!("Forwarding resize to CLI bridge");
                }
            }
        }
        ClientMessage::ListSessions => {
            if let Some(user_id) = &client_state.user_id {
                match state.session_manager.list_user_sessions(user_id).await {
                    Ok(sessions) => {
                        info!(
                            "ListSessions: returning {} sessions for user {}",
                            sessions.len(),
                            user_id
                        );
                        for s in &sessions {
                            info!(
                                "  - session: id={}, tag={}, status={:?}",
                                s.id, s.tag, s.status
                            );
                        }
                        let _ = tx.send(ServerMessage::SessionsList { sessions });
                    }
                    Err(e) => {
                        error!("Failed to list sessions: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "list_failed".to_string(),
                            message: "Failed to retrieve sessions".to_string(),
                        });
                    }
                }
            } else {
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }
        ClientMessage::StartSession { tag, profile: _ } => {
            if let Some(user_id) = &client_state.user_id {
                // For web clients creating sessions
                let machine_id = "web_client".to_string();
                let machine_name = "Web Client".to_string();
                match state
                    .session_manager
                    .create_session(user_id, &machine_id, &machine_name, &tag, "/")
                    .await
                {
                    Ok(session) => {
                        info!("Session created: {} for user {}", session.id, user_id);
                        let _ = tx.send(ServerMessage::SessionStarted { session });
                    }
                    Err(e) => {
                        error!("Failed to create session: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "create_failed".to_string(),
                            message: "Failed to create session".to_string(),
                        });
                    }
                }
            } else {
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }
        ClientMessage::StopSession { session_id } => {
            if let Some(_user_id) = &client_state.user_id {
                match state.session_manager.terminate_session(&session_id).await {
                    Ok(_) => {
                        let msg = ServerMessage::SessionStopped {
                            session_id: session_id.clone(),
                        };
                        // Notify the requester
                        let _ = tx.send(msg.clone());
                        // Notify the CLI bridge if connected
                        state.conn_manager.forward_to_cli(&session_id, msg).await;
                    }
                    Err(e) => {
                        error!("Failed to stop session: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "stop_failed".to_string(),
                            message: "Failed to stop session".to_string(),
                        });
                    }
                }
            } else {
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }
        ClientMessage::DeleteSession { session_id } => {
            if let Some(user_id) = &client_state.user_id {
                // Verify ownership first
                match state.session_manager.get_session(&session_id).await {
                    Ok(Some(session)) if session.user_id == *user_id => {
                        match state.session_manager.remove_session(&session_id).await {
                            Ok(_) => {
                                info!("Session deleted: {}", session_id);
                                // Broadcast to all connected users
                                let msg = ServerMessage::SessionDeleted {
                                    session_id: session_id.clone(),
                                };
                                // Notify all users (for UI updates)
                                state.conn_manager.broadcast_to_all_users(msg.clone()).await;
                                // Notify the specific CLI bridge to cleanup
                                state.conn_manager.forward_to_cli(&session_id, msg).await;
                            }
                            Err(e) => {
                                error!("Failed to delete session: {}", e);
                                let _ = tx.send(ServerMessage::Error {
                                    code: "delete_failed".to_string(),
                                    message: "Failed to delete session".to_string(),
                                });
                            }
                        }
                    }
                    Ok(Some(_)) => {
                        let _ = tx.send(ServerMessage::Error {
                            code: "access_denied".to_string(),
                            message: "Session belongs to another user".to_string(),
                        });
                    }
                    Ok(None) => {
                        let _ = tx.send(ServerMessage::Error {
                            code: "not_found".to_string(),
                            message: "Session not found".to_string(),
                        });
                    }
                    Err(e) => {
                        error!("Failed to get session: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "error".to_string(),
                            message: format!("Error: {}", e).to_string(),
                        });
                    }
                }
            } else {
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }
        ClientMessage::DetachSession { session_id } => {
            if client_state.is_cli_bridge {
                if client_state.session_id.as_ref() == Some(&session_id) {
                    state.conn_manager.unregister_cli(&session_id).await;
                    client_state.session_id = None;
                    client_state.is_cli_bridge = false;
                }
            } else if client_state.session_ids.remove(&session_id) {
                state.conn_manager.unregister_web(&session_id).await;
            }
        }
        ClientMessage::ListFiles { session_id, path } => {
            debug!("List files request: session={}, path={}", session_id, path);
            // TODO: Implement file listing
        }
        ClientMessage::ReadFile { session_id, path } => {
            debug!("Read file request: session={}, path={}", session_id, path);
            // TODO: Implement file reading
        }
        ClientMessage::WriteFile {
            session_id,
            path,
            content,
        } => {
            debug!(
                "Write file request: session={}, path={}, {} bytes",
                session_id,
                path,
                content.len()
            );
            // TODO: Implement file writing
        }
        ClientMessage::RegisterMachine { name, public_key } => {
            info!(
                "Register machine request: name={}, {} bytes key",
                name,
                public_key.len()
            );
            // TODO: Implement machine registration
        }
        ClientMessage::UpdateMachineStatus {
            machine_id,
            is_online,
        } => {
            debug!(
                "Update machine status: id={}, online={}",
                machine_id, is_online
            );
            // TODO: Implement machine status update
        }
        ClientMessage::RequestRemoteSession {
            machine_id,
            cwd,
            args,
        } => {
            info!(
                "RequestRemoteSession: machine_id={}, cwd={:?}, args={:?}",
                machine_id, cwd, args
            );

            // Generate a unique request ID
            let request_id = Uuid::new_v4().to_string();

            // Check if the machine has an active daemon connection
            if let Some(machine_tx) = state.conn_manager.get_machine_tx(&machine_id).await {
                // Register this client for the response
                state
                    .conn_manager
                    .register_pending_request(&request_id, tx.clone())
                    .await;

                // Send request to the machine's daemon
                let request = ServerMessage::StartRemoteSession {
                    request_id: request_id.clone(),
                    machine_id: machine_id.clone(),
                    cwd,
                    args,
                };

                if machine_tx.send(request).is_err() {
                    // Failed to send to machine
                    let _ = state.conn_manager.take_pending_request(&request_id).await;
                    let _ = tx.send(ServerMessage::Error {
                        code: "machine_unreachable".to_string(),
                        message: format!("Machine {} is not reachable", machine_id),
                    });
                } else {
                    info!(
                        "Remote session request {} sent to machine {}",
                        request_id, machine_id
                    );
                }
            } else {
                warn!("No active daemon for machine {}", machine_id);
                let _ = tx.send(ServerMessage::Error {
                    code: "machine_offline".to_string(),
                    message: format!("Machine {} is offline or has no active daemon", machine_id),
                });
            }
        }
        ClientMessage::RemoteSessionResult {
            request_id,
            success,
            session,
            error,
        } => {
            info!(
                "RemoteSessionResult: request_id={}, success={}",
                request_id, success
            );

            if success {
                if let Some(ref session_info) = session {
                    // Try to get user_id from the CLI's client_state (this connection)
                    let user_id = client_state.user_id.clone().unwrap_or_default();

                    // Register/update the machine in machines table (with latest name)
                    if let Err(e) = state
                        .machine_registry
                        .register_machine(
                            &user_id,
                            &session_info.machine_id,
                            &session_info.machine_name,
                            happy_types::Platform::current(),
                        )
                        .await
                    {
                        warn!("Failed to register machine: {}", e);
                    }

                    // Save/update the session in database
                    match state
                        .session_manager
                        .create_session_from_remote(
                            &user_id,
                            &session_info.machine_id,
                            &session_info.machine_name,
                            &session_info.tag,
                            &session_info.id,
                            &session_info.metadata.cwd,
                        )
                        .await
                    {
                        Ok(saved_session) => {
                            info!("Remote session saved to database: id={}, tag={}, machine={}, cwd={}",
                                saved_session.id, saved_session.tag, saved_session.machine_name, saved_session.metadata.cwd);

                            // Transition status from initializing to running
                            if let Err(e) = state
                                .session_manager
                                .update_session_status(&saved_session.id, SessionStatus::Running)
                                .await
                            {
                                error!("Failed to update remote session status to Running: {}", e);
                            } else {
                                info!(
                                    "Remote session {} status updated to Running",
                                    saved_session.id
                                );
                            }

                            // Broadcast SessionStarted to all users so their session list refreshes
                            let mut running_session = saved_session.clone();
                            running_session.status = SessionStatus::Running;
                            state
                                .conn_manager
                                .broadcast_to_all_users(ServerMessage::SessionStarted {
                                    session: running_session,
                                })
                                .await;
                        }
                        Err(e) => {
                            error!("Failed to save remote session to database: {}", e);
                        }
                    }
                }
            }

            // Find the pending request and send response to the web client
            if let Some(client_tx) = state.conn_manager.take_pending_request(&request_id).await {
                let response = ServerMessage::RemoteSessionResponse {
                    request_id,
                    success,
                    session,
                    error,
                };
                let _ = client_tx.send(response);
            } else {
                warn!("No pending request found for request_id {}", request_id);
            }
        }
        ClientMessage::ListMachines => {
            if let Some(user_id) = &client_state.user_id {
                // Get machines from the machines table (with latest names)
                match state.machine_registry.list_user_machines(user_id).await {
                    Ok(machines) => {
                        // Update online status based on active connections
                        let mut result = Vec::new();
                        for mut m in machines {
                            m.is_online = state.conn_manager.has_machine(&m.id).await;
                            result.push(m);
                        }
                        let _ = tx.send(ServerMessage::MachineList { machines: result });
                    }
                    Err(e) => {
                        error!("Failed to list machines: {}", e);
                        let _ = tx.send(ServerMessage::Error {
                            code: "list_failed".to_string(),
                            message: "Failed to retrieve machines".to_string(),
                        });
                    }
                }
            } else {
                let _ = tx.send(ServerMessage::Error {
                    code: "not_authenticated".to_string(),
                    message: "Please authenticate first".to_string(),
                });
            }
        }

        // Git operations - forward to CLI daemon
        ClientMessage::GetGitStatus { session_id } => {
            if let Some(user_id) = &client_state.user_id {
                // Forward to CLI daemon
                if state.conn_manager.has_cli(&session_id).await {
                    let msg = ServerMessage::GitStatusRequest {
                        session_id: session_id.clone(),
                        requester_id: client_state.connection_id.clone(),
                    };
                    state.conn_manager.forward_to_cli(&session_id, msg).await;
                } else {
                    let _ = tx.send(ServerMessage::Error {
                        code: "no_cli".to_string(),
                        message: "No CLI bridge connected for this session".to_string(),
                    });
                }
            }
        }
        ClientMessage::GetGitDiff { session_id, path } => {
            if let Some(user_id) = &client_state.user_id {
                // Forward to CLI daemon
                if state.conn_manager.has_cli(&session_id).await {
                    let msg = ServerMessage::GitDiffRequest {
                        session_id: session_id.clone(),
                        path,
                        requester_id: client_state.connection_id.clone(),
                    };
                    state.conn_manager.forward_to_cli(&session_id, msg).await;
                } else {
                    let _ = tx.send(ServerMessage::Error {
                        code: "no_cli".to_string(),
                        message: "No CLI bridge connected for this session".to_string(),
                    });
                }
            }
        }
        ClientMessage::GitCommit { session_id, message, amend } => {
            if let Some(user_id) = &client_state.user_id {
                // Forward to CLI daemon
                if state.conn_manager.has_cli(&session_id).await {
                    let msg = ServerMessage::GitCommitRequest {
                        session_id: session_id.clone(),
                        message,
                        amend,
                        requester_id: client_state.connection_id.clone(),
                    };
                    state.conn_manager.forward_to_cli(&session_id, msg).await;
                } else {
                    let _ = tx.send(ServerMessage::Error {
                        code: "no_cli".to_string(),
                        message: "No CLI bridge connected for this session".to_string(),
                    });
                }
            }
        }

        // Git operation responses from CLI daemon - forward to web clients
        ClientMessage::GitStatusResponse {
            session_id,
            branch,
            ahead,
            behind,
            modified,
            staged,
            untracked,
            conflicts,
        } => {
            if client_state.is_cli_bridge {
                let session_id_clone = session_id.clone();
                let msg = ServerMessage::GitStatus {
                    session_id,
                    branch,
                    ahead,
                    behind,
                    modified,
                    staged,
                    untracked,
                    conflicts,
                };
                state.conn_manager.broadcast_to_web(&session_id_clone, msg).await;
            }
        }
        ClientMessage::GitDiffResponse { session_id, path, diff } => {
            if client_state.is_cli_bridge {
                let session_id_clone = session_id.clone();
                let msg = ServerMessage::GitDiff {
                    session_id,
                    path,
                    diff,
                };
                state.conn_manager.broadcast_to_web(&session_id_clone, msg).await;
            }
        }
        ClientMessage::GitCommitResponse {
            session_id,
            success,
            message,
        } => {
            if client_state.is_cli_bridge {
                let session_id_clone = session_id.clone();
                let msg = ServerMessage::GitCommitResult {
                    session_id,
                    success,
                    message,
                };
                state.conn_manager.broadcast_to_web(&session_id_clone, msg).await;
            }
        }
    }

    true
}

/// Broadcast updated machine list to all connected clients for a user
async fn broadcast_machine_list(state: &AppState, user_id: &str) {
    match state.machine_registry.list_user_machines(user_id).await {
        Ok(machines) => {
            // Update online status based on active connections
            let mut result = Vec::new();
            for mut m in machines {
                m.is_online = state.conn_manager.has_machine(&m.id).await;
                result.push(m);
            }
            let msg = ServerMessage::MachineList { machines: result };
            // Send to all connections of this specific user
            state.conn_manager.send_to_user(user_id, msg).await;
            info!("Broadcasted MachineList update for user {}", user_id);
        }
        Err(e) => {
            warn!("Failed to get machines for broadcast: {}", e);
        }
    }
}

/// Send machine list to a specific user connection (for initial load)
async fn send_machine_list_to_user(
    state: &AppState,
    user_id: &str,
    tx: mpsc::UnboundedSender<ServerMessage>,
) {
    match state.machine_registry.list_user_machines(user_id).await {
        Ok(machines) => {
            // Update online status based on active connections
            let mut result = Vec::new();
            for mut m in machines {
                m.is_online = state.conn_manager.has_machine(&m.id).await;
                result.push(m);
            }
            let msg = ServerMessage::MachineList { machines: result };
            let _ = tx.send(msg);
            info!("Sent MachineList to user {} on connection", user_id);
        }
        Err(e) => {
            warn!("Failed to get machines for user {}: {}", user_id, e);
        }
    }
}
