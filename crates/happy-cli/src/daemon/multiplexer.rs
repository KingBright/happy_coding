//! Session multiplexer - manage multiple Claude sessions
//!
//! Provides tmux-like session management:
//! - List all sessions
//! - Attach/detach from sessions
//! - Create new sessions
//! - Kill sessions

use anyhow::{Context, Result};
use bytes::Bytes;
use portable_pty::PtySize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::persistence::{PersistenceManager, PersistentSession};

/// Session multiplexer - central hub for all terminal sessions
pub struct SessionMultiplexer {
    persistence: Arc<PersistenceManager>,
    /// Active WebSocket connections per session
    connections: Arc<RwLock<HashMap<String, Vec<ConnectionHandle>>>>,
}

/// Handle to a client connection
#[derive(Debug, Clone)]
pub struct ConnectionHandle {
    pub client_id: String,
    pub _session_id: String,
}

/// Request to create a new session
#[derive(Debug, Clone)]
pub struct CreateSessionRequest {
    pub id: Option<String>,
    pub tag: String,
    pub command: String,
    pub working_dir: PathBuf,
    pub env_vars: Vec<(String, String)>,
    pub size: PtySize,
}

/// Session summary for UI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub tag: String,
    pub command: String,
    pub status: SessionStatus,
    pub created_at: String,
    pub last_activity: String,
    pub has_clients: bool,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Detached,
    Exited,
}

impl SessionMultiplexer {
    pub fn new(state_dir: PathBuf) -> Result<Self> {
        let persistence = Arc::new(PersistenceManager::new(state_dir)?);

        Ok(Self {
            persistence,
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Initialize and recover existing sessions
    pub async fn initialize(&self) -> Result<()> {
        let recovered = self.persistence.recover_sessions().await?;
        info!("Recovered {} sessions from state", recovered.len());

        for metadata in recovered {
            info!(
                "  - {} (tag: {}, pid: {:?})",
                metadata.id, metadata.tag, metadata.pid
            );
            if let Err(e) = self.persistence.rehydrate_session(metadata).await {
                error!("Failed to rehydrate session: {}", e);
            }
        }

        Ok(())
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<Arc<RwLock<PersistentSession>>> {
        info!("Creating new session: tag={}", request.tag);

        let session = self
            .persistence
            .create_session(
                request.id,
                &request.tag,
                &request.command,
                request.working_dir,
                request.env_vars,
                request.size,
            )
            .await?;

        // Initialize empty connections list
        self.connections
            .write()
            .await
            .insert(session.read().await.id.clone(), Vec::new());

        info!("Session created: {}", session.read().await.id);

        Ok(session)
    }

    /// Get a session by ID
    pub async fn get_session(&self, id: &str) -> Option<Arc<RwLock<PersistentSession>>> {
        self.persistence.get_session(id).await
    }

    /// Get session by tag
    pub async fn get_session_by_tag(&self, tag: &str) -> Option<Arc<RwLock<PersistentSession>>> {
        self.persistence.get_session(tag).await
    }

    /// List all sessions with their status
    pub async fn list_sessions(&self) -> Vec<SessionSummary> {
        let mut summaries = Vec::new();
        let sessions = self.persistence.list_sessions().await;
        let connections = self.connections.read().await;

        for metadata in sessions {
            let has_clients = connections
                .get(&metadata.id)
                .map(|c| !c.is_empty())
                .unwrap_or(false);

            let status = if metadata.exit_code.is_some() {
                SessionStatus::Exited
            } else {
                // Report as running even if detached to avoid "Terminated" confusion in UI
                SessionStatus::Running
            };

            summaries.push(SessionSummary {
                id: metadata.id.clone(),
                tag: metadata.tag.clone(),
                command: metadata.command.clone(),
                status,
                created_at: metadata.created_at.to_rfc3339(),
                last_activity: metadata.last_activity.to_rfc3339(),
                has_clients,
                pid: metadata.pid,
                exit_code: metadata.exit_code,
            });
        }

        // Sort by last activity (most recent first)
        summaries.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        summaries
    }

    /// Kill a session
    pub async fn kill_session(&self, id_or_tag: &str) -> Result<()> {
        let session_id = {
            if let Some(session) = self.persistence.get_session(id_or_tag).await {
                session.read().await.id.clone()
            } else {
                anyhow::bail!("Session not found: {}", id_or_tag);
            }
        };

        info!("Killing session: {}", session_id);

        // Notify all connected clients
        if let Some(connections) = self.connections.write().await.remove(&session_id) {
            info!("Disconnecting {} clients", connections.len());
        }

        self.persistence.kill_session(&session_id).await?;

        Ok(())
    }

    /// Register a client connection to a session
    pub async fn attach_client(
        &self,
        session_id: &str,
        client_id: &str,
    ) -> Result<(tokio::sync::broadcast::Receiver<Bytes>, Vec<u8>)> {
        let session = self
            .persistence
            .get_session(session_id)
            .await
            .context("Session not found")?;

        let mut connections = self.connections.write().await;
        let client_list = connections
            .entry(session_id.to_string())
            .or_insert_with(Vec::new);

        client_list.push(ConnectionHandle {
            client_id: client_id.to_string(),
            _session_id: session_id.to_string(),
        });

        info!(
            "Client {} attached to session {} ({} clients connected)",
            client_id,
            session_id,
            client_list.len()
        );

        // Return broadcast receiver for output AND current buffer atomically
        let (rx, buffer) = {
            let s = session.read().await;
            let rx = s.subscribe_output();
            let mut buffer = s.get_buffer_contents().await;

            // Truncate to last 100KB (same logic as get_buffer)
            const MAX_BUFFER_SIZE: usize = 100 * 1024;
            if buffer.len() > MAX_BUFFER_SIZE {
                let start = buffer.len() - MAX_BUFFER_SIZE;
                buffer = buffer.split_off(start);
            }

            (rx, buffer)
        };
        Ok((rx, buffer))
    }

    /// Remove a client connection
    pub async fn detach_client(&self, session_id: &str, client_id: &str) {
        let mut connections = self.connections.write().await;

        let remaining_count = if let Some(client_list) = connections.get_mut(session_id) {
            client_list.retain(|c| c.client_id != client_id);
            let count = client_list.len();
            debug!(
                "Client {} detached from session {} ({} clients remaining)",
                client_id, session_id, count
            );
            count
        } else {
            0
        };

        // If no local clients remaining (only remote-relay or none), kill the session
        if remaining_count == 0 {
            info!(
                "No local clients remaining for session {}, killing session",
                session_id
            );
            drop(connections); // Release the lock before calling kill_session
            let _ = self.kill_session(session_id).await;
        } else if remaining_count == 1 {
            // Check if the only remaining client is remote-relay
            if let Some(client_list) = connections.get(session_id) {
                if client_list.iter().any(|c| c.client_id == "remote-relay") {
                    info!(
                        "Only remote-relay remaining for session {}, killing session",
                        session_id
                    );
                    drop(connections);
                    let _ = self.kill_session(session_id).await;
                }
            }
        }
    }

    /// Send input to a session
    pub async fn send_input(&self, session_id: &str, data: Vec<u8>) -> Result<()> {
        let session = self
            .persistence
            .get_session(session_id)
            .await
            .context("Session not found")?;

        session.read().await.write(data).await?;
        Ok(())
    }

    /// Resize a session
    pub async fn resize_session(&self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        let session = self
            .persistence
            .get_session(session_id)
            .await
            .context("Session not found")?;

        {
            let s = session.read().await;
            s.resize(cols, rows).await?;
        }
        Ok(())
    }

    /// Get session working directory
    pub async fn get_session_cwd(&self, session_id: &str) -> Result<PathBuf> {
        let session = self
            .persistence
            .get_session(session_id)
            .await
            .context("Session not found")?;

        let cwd = {
            let s = session.read().await;
            s.get_working_dir().await
        };
        Ok(cwd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let multiplexer = SessionMultiplexer::new(temp_dir.path().to_path_buf()).unwrap();

        // Initially empty
        let sessions = multiplexer.list_sessions().await;
        assert!(sessions.is_empty());

        // Create a session
        let request = CreateSessionRequest {
            id: None,
            tag: "test-session".to_string(),
            command: "/bin/sleep".to_string(),
            working_dir: PathBuf::from("/tmp"),
            env_vars: vec![],
            size: PtySize {
                cols: 80,
                rows: 24,
                pixel_width: 0,
                pixel_height: 0,
            },
        };

        let session = multiplexer.create_session(request).await.unwrap();
        assert_eq!(session.read().await.tag, "test-session");

        // Should be in list
        let sessions = multiplexer.list_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].tag, "test-session");
    }
}
