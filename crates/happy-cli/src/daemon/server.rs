//! Daemon WebSocket server for browser-to-session communication
//!
//! Allows browsers to:
//! - List all active sessions
//! - Create new sessions
//! - Attach to existing sessions
//! - Send input and receive output
//! - Resize terminals
//! - Kill sessions

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{sink::SinkExt, stream::StreamExt};
use portable_pty::PtySize;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::multiplexer::{CreateSessionRequest, SessionMultiplexer, SessionSummary};

/// WebSocket server for daemon control
pub struct DaemonServer {
    multiplexer: Arc<SessionMultiplexer>,
    bind_addr: SocketAddr,
}

/// Client protocol messages (from browser to daemon)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// List all sessions
    ListSessions,
    /// Create a new session
    CreateSession {
        tag: String,
        #[serde(default = "default_command")]
        command: String,
        #[serde(default)]
        working_dir: Option<PathBuf>,
        #[serde(default)]
        env_vars: Vec<(String, String)>,
        #[serde(default = "default_size")]
        size: TermSize,
    },
    /// Attach to an existing session
    AttachSession { session_id: String },
    /// Send input to the current session
    Input { data: String },
    /// Resize terminal
    Resize { cols: u16, rows: u16 },
    /// Kill a session
    KillSession { session_id: String },
    /// Detach from current session (but keep it running)
    Detach,
    /// Get session info
    SessionInfo { session_id: String },
}

fn default_command() -> String {
    "claude".to_string()
}

fn default_size() -> TermSize {
    TermSize { cols: 80, rows: 24 }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

impl From<TermSize> for PtySize {
    fn from(size: TermSize) -> Self {
        PtySize {
            cols: size.cols,
            rows: size.rows,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Server protocol messages (from daemon to browser)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Connection established
    Connected { client_id: String },
    /// Error occurred
    Error { message: String },
    /// Session list
    SessionsList { sessions: Vec<SessionSummary> },
    /// Session created
    SessionCreated { session_id: String, tag: String },
    /// Session killed
    SessionKilled { session_id: String },
    /// Attached to session
    SessionAttached {
        session_id: String,
        /// Initial buffer content
        buffer: String,
    },
    /// Detached from session
    SessionDetached { session_id: String },
    /// Terminal output
    #[allow(dead_code)]
    Output { data: String },
    /// Session info
    SessionInfo {
        session_id: String,
        tag: String,
        status: String,
        connected_clients: usize,
    },
}

/// Client connection state
struct ClientState {
    client_id: String,
    /// Currently attached session
    session_id: Option<String>,
    /// Output receiver (for detached sessions)
    #[allow(dead_code)]
    output_rx: Option<tokio::sync::broadcast::Receiver<Bytes>>,
}

impl DaemonServer {
    pub fn new(multiplexer: Arc<SessionMultiplexer>, bind_addr: SocketAddr) -> Self {
        Self {
            multiplexer,
            bind_addr,
        }
    }

    /// Start the WebSocket server
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("Daemon WebSocket server listening on {}", self.bind_addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let multiplexer = self.multiplexer.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, addr, multiplexer).await {
                    error!("Connection error from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    multiplexer: Arc<SessionMultiplexer>,
) -> Result<()> {
    debug!("New connection from {}", addr);

    let ws_stream = accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Generate client ID
    let client_id = format!("client-{}", Uuid::new_v4());
    let mut state = ClientState {
        client_id: client_id.clone(),
        session_id: None,
        output_rx: None,
    };

    // Send connected message
    let connected_msg = ServerMessage::Connected {
        client_id: client_id.clone(),
    };
    ws_tx
        .send(Message::Text(serde_json::to_string(&connected_msg)?))
        .await?;

    loop {
        if state.output_rx.is_none() {
            let msg = match ws_rx.next().await {
                Some(msg) => msg?,
                None => break,
            };
            match msg {
                Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        if let Err(e) =
                            handle_client_message(&client_msg, &mut state, &multiplexer, &mut ws_tx)
                                .await
                        {
                            let error_msg = ServerMessage::Error {
                                message: e.to_string(),
                            };
                            ws_tx
                                .send(Message::Text(serde_json::to_string(&error_msg)?))
                                .await?;
                        }
                    }
                    Err(e) => {
                        warn!("Invalid message from {}: {}", addr, e);
                        let error_msg = ServerMessage::Error {
                            message: format!("Invalid message: {}", e),
                        };
                        ws_tx
                            .send(Message::Text(serde_json::to_string(&error_msg)?))
                            .await?;
                    }
                },
                Message::Binary(data) => {
                    if let Some(session_id) = &state.session_id {
                        if let Err(e) = multiplexer.send_input(session_id, data).await {
                            let response = ServerMessage::Error {
                                message: format!("Failed to send input: {}", e),
                            };
                            ws_tx
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;
                        }
                    } else {
                        let response = ServerMessage::Error {
                            message: "Not attached to any session".to_string(),
                        };
                        ws_tx
                            .send(Message::Text(serde_json::to_string(&response)?))
                            .await?;
                    }
                }
                Message::Close(_) => {
                    debug!("Connection closed from {}", addr);
                    break;
                }
                _ => {}
            }
            continue;
        }

        let mut output_rx = state.output_rx.take().unwrap();
        tokio::select! {
            msg = ws_rx.next() => {
                let msg = match msg {
                    Some(msg) => msg?,
                    None => break,
                };
                match msg {
                    Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            if let Err(e) = handle_client_message(&client_msg, &mut state, &multiplexer, &mut ws_tx).await {
                                let error_msg = ServerMessage::Error {
                                    message: e.to_string(),
                                };
                                ws_tx
                                    .send(Message::Text(serde_json::to_string(&error_msg)?))
                                    .await?;
                            }
                        }
                        Err(e) => {
                            warn!("Invalid message from {}: {}", addr, e);
                            let error_msg = ServerMessage::Error {
                                message: format!("Invalid message: {}", e),
                            };
                            ws_tx
                                .send(Message::Text(serde_json::to_string(&error_msg)?))
                                .await?;
                        }
                    },
                    Message::Binary(data) => {
                        if let Some(session_id) = &state.session_id {
                            if let Err(e) = multiplexer.send_input(session_id, data).await {
                                let response = ServerMessage::Error {
                                    message: format!("Failed to send input: {}", e),
                                };
                                ws_tx
                                    .send(Message::Text(serde_json::to_string(&response)?))
                                    .await?;
                            }
                        } else {
                            let response = ServerMessage::Error {
                                message: "Not attached to any session".to_string(),
                            };
                            ws_tx
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;
                        }
                    }
                    Message::Close(_) => {
                        debug!("Connection closed from {}", addr);
                        break;
                    }
                    _ => {}
                }
                if state.output_rx.is_none() && state.session_id.is_some() {
                    state.output_rx = Some(output_rx);
                }
            }
            output = output_rx.recv() => {
                state.output_rx = Some(output_rx);
                if let Ok(data) = output {
                    let response = ServerMessage::Output {
                        data: String::from_utf8_lossy(&data).to_string(),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
            }
        }
    }

    // Cleanup: detach from any session
    if let Some(session_id) = &state.session_id {
        multiplexer.detach_client(session_id, &client_id).await;
    }

    Ok(())
}

async fn handle_client_message(
    msg: &ClientMessage,
    state: &mut ClientState,
    multiplexer: &Arc<SessionMultiplexer>,
    ws_tx: &mut futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message,
    >,
) -> Result<()> {
    match msg {
        ClientMessage::ListSessions => {
            let sessions = multiplexer.list_sessions().await;
            let response = ServerMessage::SessionsList { sessions };
            ws_tx
                .send(Message::Text(serde_json::to_string(&response)?))
                .await?;
        }

        ClientMessage::CreateSession {
            tag,
            command,
            working_dir,
            env_vars,
            size,
        } => {
            let request = CreateSessionRequest {
                id: None,
                tag: tag.clone(),
                command: command.clone(),
                working_dir: working_dir.clone().unwrap_or_else(|| {
                    // If no cwd specified, use user's HOME directory, not daemon's current_dir
                    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
                }),
                env_vars: env_vars.clone(),
                size: (*size).into(),
            };

            match multiplexer.create_session(request).await {
                Ok(session) => {
                    let session = session.read().await;
                    let response = ServerMessage::SessionCreated {
                        session_id: session.id.clone(),
                        tag: session.tag.clone(),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
                Err(e) => {
                    let response = ServerMessage::Error {
                        message: format!("Failed to create session: {}", e),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
            }
        }

        ClientMessage::AttachSession {
            session_id: id_or_tag,
        } => {
            // Resolve ID if tag provided
            debug!("Attempting to resolve session: {}", id_or_tag);
            let session_id = if let Some(session) = multiplexer.get_session(&id_or_tag).await {
                let guard = session.read().await;
                let id = guard.id.clone();
                debug!("Found session by id/tag (direct): {} -> {}", id_or_tag, id);
                id
            } else {
                debug!("Session not found: {}", id_or_tag);
                let response = ServerMessage::Error {
                    message: format!("Session not found: {}", id_or_tag),
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
                return Ok(());
            };

            // Detach from current session if any
            if let Some(current_id) = &state.session_id {
                multiplexer
                    .detach_client(current_id, &state.client_id)
                    .await;
            }

            // Attach to new session and get buffer atomically
            match multiplexer
                .attach_client(&session_id, &state.client_id)
                .await
            {
                Ok((output_rx, buffer)) => {
                    state.session_id = Some(session_id.clone());
                    state.output_rx = Some(output_rx);

                    let response = ServerMessage::SessionAttached {
                        session_id: session_id.clone(),
                        buffer: String::from_utf8_lossy(&buffer).to_string(),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
                Err(e) => {
                    let response = ServerMessage::Error {
                        message: format!("Failed to attach: {}", e),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
            }
        }

        ClientMessage::Input { data } => {
            if let Some(session_id) = &state.session_id {
                // Convert string to bytes (simple approach)
                let bytes = data.as_bytes().to_vec();
                if let Err(e) = multiplexer.send_input(session_id, bytes).await {
                    let response = ServerMessage::Error {
                        message: format!("Failed to send input: {}", e),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
            } else {
                let response = ServerMessage::Error {
                    message: "Not attached to any session".to_string(),
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
            }
        }

        ClientMessage::Resize { cols, rows } => {
            if let Some(session_id) = &state.session_id {
                if let Err(e) = multiplexer.resize_session(session_id, *cols, *rows).await {
                    let response = ServerMessage::Error {
                        message: format!("Failed to resize: {}", e),
                    };
                    ws_tx
                        .send(Message::Text(serde_json::to_string(&response)?))
                        .await?;
                }
            }
        }

        ClientMessage::KillSession { session_id } => {
            if let Err(e) = multiplexer.kill_session(session_id).await {
                let response = ServerMessage::Error {
                    message: format!("Failed to kill session: {}", e),
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
            } else {
                // If we were attached to this session, detach
                if state.session_id.as_ref() == Some(session_id) {
                    state.session_id = None;
                    state.output_rx = None;
                }

                let response = ServerMessage::SessionKilled {
                    session_id: session_id.clone(),
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
            }
        }

        ClientMessage::Detach => {
            if let Some(session_id) = &state.session_id {
                multiplexer
                    .detach_client(session_id, &state.client_id)
                    .await;

                let response = ServerMessage::SessionDetached {
                    session_id: session_id.clone(),
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;

                state.session_id = None;
                state.output_rx = None;
            }
        }

        ClientMessage::SessionInfo { session_id } => {
            if let Some(session) = multiplexer.get_session(session_id).await {
                let metadata = session.read().await.get_metadata().await;
                let response = ServerMessage::SessionInfo {
                    session_id: metadata.id,
                    tag: metadata.tag,
                    status: if metadata.pid.is_some() {
                        "running".to_string()
                    } else {
                        "exited".to_string()
                    },
                    connected_clients: 0, // TODO: track this
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
            } else {
                let response = ServerMessage::Error {
                    message: "Session not found".to_string(),
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
            }
        }
    }

    Ok(())
}

/// Start the daemon server with the given multiplexer
pub async fn start_daemon_server(
    multiplexer: Arc<SessionMultiplexer>,
    bind_addr: SocketAddr,
) -> Result<()> {
    let server = DaemonServer::new(multiplexer, bind_addr);
    server.run().await
}
