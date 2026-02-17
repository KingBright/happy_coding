use crate::daemon::bridge::RemoteRelayBridge;
use crate::daemon::multiplexer::{CreateSessionRequest, SessionMultiplexer};
use anyhow::Result;
use portable_pty::PtySize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Manages multiple PTY sessions running within the daemon
/// and their connections to the Remote Relay.
#[derive(Clone)]
pub struct DaemonSessionManager {
    multiplexer: Arc<SessionMultiplexer>,
    // Track relay bridges
    relays: Arc<RwLock<HashMap<String, RelayHandle>>>,
}

struct RelayHandle {
    // We might need a way to stop the bridge
    stop_tx: tokio::sync::broadcast::Sender<()>,
}

impl DaemonSessionManager {
    pub fn new(state_dir: PathBuf) -> Result<Self> {
        let multiplexer = Arc::new(SessionMultiplexer::new(state_dir)?);

        Ok(Self {
            multiplexer,
            relays: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn multiplexer(&self) -> Arc<SessionMultiplexer> {
        self.multiplexer.clone()
    }

    pub async fn start_session(
        &self,
        id: Option<String>,
        tag: String,
        token: String,
        server_url: String,
        cwd: String,
    ) -> Result<String> {
        // Keep a clone of tag for later use in bridge
        let tag_for_bridge = tag.clone();

        // 1. Check if session with this tag already exists AND is running
        let session_id = {
            if let Some(session) = self.multiplexer.get_session_by_tag(&tag).await {
                let (existing_id, is_running) = {
                    let guard = session.read().await;
                    let metadata = guard.get_metadata().await;
                    (guard.id.clone(), metadata.pid.is_some())
                };

                if is_running {
                    info!(
                        "Session with tag {} already exists and is running: {}",
                        tag, existing_id
                    );
                    existing_id
                } else {
                    // Session exists but process is dead - kill it and create new
                    info!(
                        "Session with tag {} exists but process is dead, recreating: {}",
                        tag, existing_id
                    );
                    let _ = self.multiplexer.kill_session(&existing_id).await;
                    // Fall through to create new session with provided cwd
                    self.create_new_session(Some(existing_id), tag, cwd.clone())
                        .await?
                }
            } else {
                // 2. Create new session with provided cwd
                self.create_new_session(id, tag, cwd.clone()).await?
            }
        };

        // 3. Start Relay Bridge if not running
        self.clone()
            .start_relay_bridge(session_id.clone(), tag_for_bridge, token, server_url, cwd)
            .await?;

        Ok(session_id)
    }

    pub fn start_relay_bridge(
        self,
        session_id: String,
        tag: String,
        token: String,
        server_url: String,
        cwd: String,
    ) -> futures::future::BoxFuture<'static, Result<()>> {
        use futures::FutureExt;

        // Clone necessary data for the async block
        let relays = self.relays.clone();
        // We need manager for the inner task, but we can't capture `self` in `async move` if we want 'static lifetime unless we clone `self` too.
        // But the return type is bound by 'self lifetime.
        // Let's capture `self` (reference) in the async block.
        // But `async move` will take ownership of `session_id` etc.
        // `self` is a reference. `async move` captures the reference? Yes.

        let manager = self; // Capture a clone to avoid borrowing self for too long?
                            // Actually, start_session calls this.
                            // DaemonSessionManager is cheap to clone.

        async move {
            let mut relays = relays.write().await;
            if !relays.contains_key(&session_id) {
                info!("Starting relay for session {}", session_id);

                let (stop_tx, mut stop_rx) = tokio::sync::broadcast::channel(1);

                // Channel for spawning new bridges from within this bridge
                let (spawner_tx, mut spawner_rx) =
                    tokio::sync::mpsc::unbounded_channel::<(String, String, String)>();

                // Spawn task to handle bridge spawning requests
                let manager_clone = manager.clone();
                let token_clone = token.clone();
                let server_url_clone = server_url.clone();
                tokio::spawn(async move {
                    while let Some((new_id, new_tag, new_cwd)) = spawner_rx.recv().await {
                        info!(
                            "Spawning additional bridge for session {} ({})",
                            new_id, new_tag
                        );

                        let manager = manager_clone.clone();
                        let token = token_clone.clone();
                        let server_url = server_url_clone.clone();

                        tokio::spawn(async move {
                            let id_for_err = new_id.clone();
                            if let Err(e) = manager
                                .start_relay_bridge(new_id, new_tag, token, server_url, new_cwd)
                                .await
                            {
                                error!("Failed to spawn bridge for session {}: {}", id_for_err, e);
                            }
                        });
                    }
                });

                // Load machine info from settings
                let settings = crate::config::SettingsManager::load().ok();
                let machine_id = settings
                    .as_ref()
                    .map(|s| s.machine_id.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let machine_name = happy_core::utils::get_machine_name();

                let bridge = RemoteRelayBridge::new(
                    session_id.clone(),
                    tag,
                    token,
                    server_url,
                    cwd,
                    machine_id,
                    machine_name,
                    Some(spawner_tx),
                );
                let multiplexer = manager.multiplexer.clone(); // Access via manager clone
                let session_id_clone = session_id.clone();

                tokio::spawn(async move {
                    info!("Starting bridge for session {}", session_id_clone);
                    tokio::select! {
                        res = bridge.run(multiplexer) => {
                            if let Err(e) = res {
                                error!("Bridge for session {} error: {}", session_id_clone, e);
                            }
                        }
                        _ = stop_rx.recv() => {
                            info!("Stopping bridge for session {}", session_id_clone);
                        }
                    }
                    info!("Bridge for session {} exited", session_id_clone);
                });

                relays.insert(session_id.clone(), RelayHandle { stop_tx });
            }
            Ok(())
        }
        .boxed()
    }

    pub async fn stop_session(&self, session_id: &str) -> Result<()> {
        let mut relays = self.relays.write().await;
        if let Some(handle) = relays.remove(session_id) {
            let _ = handle.stop_tx.send(());
            info!("Stopped relay for session {}", session_id);
        }

        // Also kill the actual session?
        // Typically "stop session" implies killing the process.
        self.multiplexer.kill_session(session_id).await?;

        Ok(())
    }

    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.multiplexer.list_sessions().await;
        sessions.into_iter().map(|s| s.id).collect()
    }

    /// Helper to create a new session
    async fn create_new_session(
        &self,
        id: Option<String>,
        tag: String,
        cwd: String,
    ) -> Result<String> {
        let request = CreateSessionRequest {
            id,
            tag: tag.clone(),
            command: "claude".to_string(), // TODO: Make configurable via RPC
            working_dir: std::path::PathBuf::from(&cwd),
            env_vars: vec![
                ("HAPPY_SESSION_TAG".to_string(), tag.clone()),
                ("TERM".to_string(), "xterm-256color".to_string()),
            ],
            size: PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        };

        let session = self.multiplexer.create_session(request).await?;
        let id = {
            let guard = session.read().await;
            guard.id.clone()
        };
        info!("Created new session: {} (tag: {})", id, tag);
        Ok(id)
    }

    pub async fn recover_sessions(&self) -> Result<()> {
        info!("DEBUG: recover_sessions called");
        // 1. Recover sessions from persistence
        if let Err(e) = self.multiplexer.initialize().await {
            error!("Failed to recover sessions: {}", e);
            return Err(e);
        }

        // 2. Load settings for credentials
        let settings = crate::config::SettingsManager::load().unwrap_or_default();
        let token = settings.access_token.unwrap_or_default();
        let server_url = settings.server_url.clone();

        if token.is_empty() {
            info!("No access token found, skipping remote bridge recovery");
            return Ok(());
        }

        // 3. Iterate over active sessions and restart bridges
        let sessions = self.multiplexer.list_sessions().await;
        info!("DEBUG: Found {} sessions in persistence", sessions.len());
        let mut recovered_count = 0;

        for session in sessions {
            info!(
                "DEBUG: Checking session {} (pid: {:?})",
                session.id, session.pid
            );
            // Check if session process is actually running (pid is present)
            if session.pid.is_some() {
                info!(
                    "Recovering bridge for session: {} ({})",
                    session.id, session.tag
                );

                // Get full metadata to retrieve cwd
                let cwd = if let Some(s) = self.multiplexer.get_session(&session.id).await {
                    s.read()
                        .await
                        .get_metadata()
                        .await
                        .working_dir
                        .to_string_lossy()
                        .to_string()
                } else {
                    "/".to_string()
                };

                // Spawn bridge
                let manager = self.clone();
                let session_id = session.id.clone();
                let tag = session.tag.clone();
                let token = token.clone();
                let server_url = server_url.clone();

                tokio::spawn(async move {
                    if let Err(e) = manager
                        .start_relay_bridge(session_id.clone(), tag, token, server_url, cwd)
                        .await
                    {
                        error!("Failed to recover bridge for session {}: {}", session_id, e);
                    }
                });
                recovered_count += 1;
            }
        }

        if recovered_count > 0 {
            info!("Recovered bridges for {} sessions", recovered_count);
        }

        Ok(())
    }
}
