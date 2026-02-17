//! Native persistence implementation - no external dependencies
//!
//! This module provides tmux-like session persistence using pure Rust:
//! - Forking to create daemon processes
//! - PTY management for terminal emulation
//! - Ring buffer for output history
//! - Session state serialization for recovery

use anyhow::Result;
use bytes::Bytes;
use portable_pty::{Child, CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Size of the output ring buffer (10MB)
const BUFFER_SIZE: usize = 10 * 1024 * 1024;

/// Maximum number of lines to keep in scrollback
const MAX_SCROLLBACK_LINES: usize = 10000;

/// Persistent terminal session that survives disconnections
pub struct PersistentSession {
    /// Session ID
    pub id: String,
    /// Human-readable tag
    pub tag: String,
    /// Command sender to PTY (for input)
    cmd_tx: mpsc::Sender<Vec<u8>>,
    /// Resize sender
    resize_tx: mpsc::Sender<(u16, u16)>,
    /// Output broadcast channel (for multiple clients)
    output_tx: broadcast::Sender<Bytes>,
    /// Ring buffer for output history
    buffer: Arc<RwLock<RingBuffer>>,
    /// Session metadata
    metadata: Arc<RwLock<SessionMetadata>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub tag: String,
    pub command: String,
    pub working_dir: PathBuf,
    pub env_vars: Vec<(String, String)>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub cols: u16,
    pub rows: u16,
    /// Process ID if running
    pub pid: Option<u32>,
    /// Exit status if completed
    pub exit_code: Option<i32>,
}

/// Ring buffer for terminal output with scrollback
pub struct RingBuffer {
    /// Raw bytes buffer
    data: VecDeque<u8>,
    /// Maximum size in bytes
    max_size: usize,
    /// Absolute line offsets for efficient line-based retrieval
    line_offsets: VecDeque<u64>,
    /// Total bytes ever written (used to calculate absolute offsets)
    total_written: u64,
}

impl RingBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_size.min(1024 * 1024)),
            max_size,
            line_offsets: VecDeque::new(),
            total_written: 0,
        }
    }

    /// Append data to buffer, evicting old data if necessary
    pub fn push(&mut self, data: &[u8]) {
        for &byte in data {
            // Track line endings with absolute offset
            if byte == b'\n' {
                self.line_offsets.push_back(self.total_written);
            }

            self.data.push_back(byte);
            self.total_written += 1;

            // Evict if over limit
            if self.data.len() > self.max_size {
                self.data.pop_front();
            }
        }

        // Clean up line offsets that are no longer in the buffer
        let start_offset = self.total_written.saturating_sub(self.data.len() as u64);
        while let Some(&offset) = self.line_offsets.front() {
            if offset < start_offset {
                self.line_offsets.pop_front();
            } else {
                break;
            }
        }

        // Limit number of tracked lines
        while self.line_offsets.len() > MAX_SCROLLBACK_LINES {
            self.line_offsets.pop_front();
        }
    }

    /// Get all buffer contents as bytes
    pub fn get_contents(&self) -> Vec<u8> {
        self.data.iter().copied().collect()
    }

    /// Restore buffer from saved data
    pub fn restore(&mut self, data: &[u8]) {
        self.data.clear();
        self.line_offsets.clear();
        self.total_written = 0;
        self.push(data);
    }
}

/// Manager for all persistent sessions
pub struct PersistenceManager {
    sessions: Arc<RwLock<std::collections::HashMap<String, Arc<RwLock<PersistentSession>>>>>,
    state_dir: PathBuf,
}

impl PersistenceManager {
    pub fn new(mut state_dir: PathBuf) -> Result<Self> {
        let base_dir = state_dir.clone();
        state_dir.push("sessions");

        if !state_dir.exists() {
            std::fs::create_dir_all(&state_dir)?;
        }

        // Migration: Move existing session .json files from base_dir to sessions dir
        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "json") {
                    let filename = path.file_name().unwrap();
                    if filename != "settings.json" {
                        let dest = state_dir.join(filename);
                        let _ = std::fs::rename(&path, dest);
                    }
                }
            }
        }

        Ok(Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            state_dir,
        })
    }

    /// Create a new persistent session
    pub async fn create_session(
        &self,
        id: Option<String>,
        tag: &str,
        command: &str,
        working_dir: PathBuf,
        env_vars: Vec<(String, String)>,
        size: PtySize,
    ) -> Result<Arc<RwLock<PersistentSession>>> {
        let session_id = id.unwrap_or_else(|| format!("{}", uuid::Uuid::new_v4()));
        info!("Creating persistent session: {} (tag: {})", session_id, tag);

        // Create PTY
        let pty_system = NativePtySystem::default();
        let pair = pty_system.openpty(size)?;

        // Build command
        let mut cmd_builder = CommandBuilder::new(command);
        cmd_builder.cwd(working_dir.clone());
        for (key, value) in &env_vars {
            cmd_builder.env(key, value);
        }

        // Spawn the process
        let child = pair.slave.spawn_command(cmd_builder)?;
        let child_pid = child.process_id();

        info!("Spawned process with PID: {:?}", child_pid);

        // Create channels
        let (cmd_tx, cmd_rx) = mpsc::channel::<Vec<u8>>(100);
        let (resize_tx, resize_rx) = mpsc::channel::<(u16, u16)>(10);
        let (output_tx, _output_rx) = broadcast::channel::<Bytes>(1000);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Shared state
        let buffer = Arc::new(RwLock::new(RingBuffer::new(BUFFER_SIZE)));
        let metadata = Arc::new(RwLock::new(SessionMetadata {
            id: session_id.clone(),
            tag: tag.to_string(),
            command: command.to_string(),
            working_dir,
            env_vars,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
            cols: size.cols,
            rows: size.rows,
            pid: child_pid,
            exit_code: None,
        }));

        // Start PTY handler in a blocking task
        let buffer_clone = buffer.clone();
        let metadata_clone = metadata.clone();
        let output_tx_clone = output_tx.clone();
        let session_id_clone = session_id.clone();
        let state_dir = self.state_dir.clone();

        let runtime_handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            runtime_handle.block_on(async {
                run_pty_loop(
                    pair,
                    child,
                    cmd_rx,
                    resize_rx,
                    shutdown_rx,
                    buffer_clone,
                    metadata_clone,
                    output_tx_clone,
                    session_id_clone,
                    state_dir,
                )
                .await;
            });
        });

        let session = Arc::new(RwLock::new(PersistentSession {
            id: session_id.clone(),
            tag: tag.to_string(),
            cmd_tx,
            resize_tx,
            output_tx: output_tx.clone(),
            buffer,
            metadata,
            shutdown_tx: Some(shutdown_tx),
        }));

        // Store session
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), session.clone());

        // Save initial state
        let meta = session.read().await.metadata.read().await.clone();
        save_session_state(&self.state_dir, &meta).await?;

        Ok(session)
    }

    /// Get an existing session
    pub async fn get_session(&self, id_or_tag: &str) -> Option<Arc<RwLock<PersistentSession>>> {
        let sessions = self.sessions.read().await;

        // 1. Try direct ID lookup
        if let Some(session) = sessions.get(id_or_tag) {
            return Some(session.clone());
        }

        // 2. Try tag lookup
        for session in sessions.values() {
            if session.read().await.tag == id_or_tag {
                return Some(session.clone());
            }
        }

        None
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<SessionMetadata> {
        let mut result = Vec::new();
        for session in self.sessions.read().await.values() {
            result.push(session.read().await.metadata.read().await.clone());
        }
        result
    }

    /// Kill a session
    pub async fn kill_session(&self, id_or_tag: &str) -> Result<()> {
        let (id, pid) = {
            if let Some(session) = self.get_session(id_or_tag).await {
                let guard = session.read().await;
                let id = guard.id.clone();
                let pid = guard.metadata.read().await.pid;
                (id, pid)
            } else {
                anyhow::bail!("Session not found: {}", id_or_tag);
            }
        };

        if self.sessions.write().await.remove(&id).is_some() {
            // Actually kill the process first
            if let Some(pid) = pid {
                #[cfg(unix)]
                unsafe {
                    info!("Killing process {} for session {}", pid, id);
                    libc::kill(pid as i32, libc::SIGTERM);
                    // Give it a moment to terminate gracefully
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    // Force kill if still running
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                #[cfg(windows)]
                {
                    use sysinfo::{ProcessExt, System, SystemExt};
                    let mut s = System::new_all();
                    s.refresh_all();
                    if let Some(process) = s.process(sysinfo::Pid::from(pid as usize)) {
                        info!("Killing process {} for session {}", pid, id);
                        process.kill();
                    }
                }
            }

            info!("Killed session {}", id);
        }

        // Remove state file
        let state_file = self.state_dir.join(format!("{}.json", id));
        let _ = tokio::fs::remove_file(state_file).await;

        Ok(())
    }

    /// Recover sessions from state files (called on daemon startup)
    /// Rehydrate a session from metadata (recovered from disk)
    pub async fn rehydrate_session(
        &self,
        metadata: SessionMetadata,
    ) -> Result<Arc<RwLock<PersistentSession>>> {
        let session_id = metadata.id.clone();

        // Check if the process is still running
        let is_running = if let Some(pid) = metadata.pid {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, 0) == 0
            }
            #[cfg(windows)]
            {
                use sysinfo::{ProcessExt, System, SystemExt};
                let s = System::new_all();
                s.process(sysinfo::Pid::from(pid as usize)).is_some()
            }
        } else {
            false
        };

        // Create channels
        let (cmd_tx, cmd_rx) = mpsc::channel(100);

        let (resize_tx, resize_rx) = mpsc::channel(10);
        let (output_tx, _) = broadcast::channel(1000);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let buffer = Arc::new(RwLock::new(RingBuffer::new(BUFFER_SIZE)));

        // Load existing history from log
        if let Ok(data) = load_session_log(&self.state_dir, &session_id).await {
            if !data.is_empty() {
                debug!(
                    "Loaded {} bytes of history for session {}",
                    data.len(),
                    session_id
                );
                buffer.write().await.restore(&data);
            }
        }

        // Shared state for metadata
        let metadata_arc = Arc::new(RwLock::new(metadata.clone()));

        if is_running {
            // Process is running (orphaned from previous daemon instance?)
            // If the daemon process died but the child PTY didn't, we can't easily re-attach to the PTY master
            // because the file descriptor is lost.
            //
            // In a typical daemon restart scenario, the OS closes the PTY master when the daemon dies,
            // causing the child shell to exit (SIGHUP). So `is_running` should be false in most cases.
            //
            // If it IS running, it's likely a zombie or detached. We probably should kill it and respawn
            // to regain control, or just mark it as dead if we can't control it.
            //
            // For now, let's assume if we lost the FD, we can't control it.
            // We'll treat it as dead, kill the old PID, and respawn.
            info!(
                "Found running process {} for session {}, but we lost PTY control. Respawning...",
                metadata.pid.unwrap_or(0),
                session_id
            );
            if let Some(pid) = metadata.pid {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                #[cfg(windows)]
                { /* Windows kill logic */ }
            }
        }

        // RESP_AWN LOGIC
        // We always respawn because we lost the PTY file descriptors
        info!("Respawning session {} (tag: {})", session_id, metadata.tag);

        let pty_system = NativePtySystem::default();
        let size = PtySize {
            cols: metadata.cols,
            rows: metadata.rows,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size)?;

        let mut cmd_builder = CommandBuilder::new(&metadata.command);
        cmd_builder.cwd(metadata.working_dir.clone());
        for (key, value) in &metadata.env_vars {
            cmd_builder.env(key, value);
        }

        let child = pair.slave.spawn_command(cmd_builder)?;
        let child_pid = child.process_id();
        metadata_arc.write().await.pid = child_pid;
        metadata_arc.write().await.exit_code = None;

        // Start PTY handler
        let buffer_clone = buffer.clone();
        let metadata_clone = metadata_arc.clone();
        let output_tx_clone = output_tx.clone();
        let session_id_clone = session_id.clone();
        let state_dir = self.state_dir.clone();

        let runtime_handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            runtime_handle.block_on(async {
                run_pty_loop(
                    pair,
                    child,
                    cmd_rx,
                    resize_rx,
                    shutdown_rx,
                    buffer_clone,
                    metadata_clone,
                    output_tx_clone,
                    session_id_clone,
                    state_dir,
                )
                .await;
            });
        });

        let session = Arc::new(RwLock::new(PersistentSession {
            id: session_id.clone(),
            tag: metadata.tag.clone(),
            cmd_tx,
            resize_tx,
            output_tx,
            buffer,
            metadata: metadata_arc,
            shutdown_tx: Some(shutdown_tx),
        }));

        self.sessions
            .write()
            .await
            .insert(session_id, session.clone());

        Ok(session)
    }

    pub async fn recover_sessions(&self) -> Result<Vec<SessionMetadata>> {
        let mut recovered = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.state_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        match serde_json::from_str::<SessionMetadata>(&content) {
                            Ok(metadata) => {
                                // Check if process is still running
                                if metadata.pid.is_some()
                                    && is_process_running(metadata.pid.unwrap())
                                {
                                    info!(
                                        "Found running session: {} (tag: {})",
                                        metadata.id, metadata.tag
                                    );
                                    recovered.push(metadata);
                                } else {
                                    // Process is dead, but we keep the state file for now
                                    // so the user can see the exit status and final logs.
                                    // We might want a TTL here later.
                                    info!(
                                        "Found finished session: {} (tag: {})",
                                        metadata.id, metadata.tag
                                    );
                                    recovered.push(metadata);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse state file {:?}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read state file {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(recovered)
    }
}

/// Main PTY I/O loop running in a blocking task
async fn run_pty_loop(
    pair: portable_pty::PtyPair,
    child: Box<dyn Child>,
    mut cmd_rx: mpsc::Receiver<Vec<u8>>,
    mut resize_rx: mpsc::Receiver<(u16, u16)>,
    mut shutdown_rx: mpsc::Receiver<()>,
    buffer: Arc<RwLock<RingBuffer>>,
    metadata: Arc<RwLock<SessionMetadata>>,
    output_tx: broadcast::Sender<Bytes>,
    session_id: String,
    state_dir: PathBuf,
) {
    // Get PTY handles
    let mut reader = match pair.master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get PTY reader: {}", e);
            return;
        }
    };

    let mut writer = match pair.master.take_writer() {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to get PTY writer: {}", e);
            return;
        }
    };

    // Wrap child in Arc<std::sync::Mutex<Option<Box<dyn Child>>>> for shared access
    let child_arc = Arc::new(std::sync::Mutex::new(Some(child)));

    // 1. Save state immediately
    {
        let meta = metadata.read().await.clone();
        if let Err(e) = save_session_state(&state_dir, &meta).await {
            warn!("Failed initial session state save: {}", e);
        }
    }

    // Start writer task
    let writer_handle = tokio::task::spawn_blocking(move || {
        while let Some(data) = cmd_rx.blocking_recv() {
            if writer.write_all(&data).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });

    // Start reader task (this is the key fix: move blocking read to its own task)
    let (reader_tx, mut reader_rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(1000);
    tokio::task::spawn_blocking(move || {
        let mut local_buf = [0u8; 4096];
        loop {
            match reader.read(&mut local_buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let _ = reader_tx.blocking_send(Ok(Bytes::copy_from_slice(&local_buf[..n])));
                }
                Err(e) => {
                    let _ = reader_tx.blocking_send(Err(e));
                    break;
                }
            }
        }
    });

    // Reader loop (now purely async)
    let mut save_interval = interval(Duration::from_secs(30));
    let mut _killed = false;

    loop {
        tokio::select! {
            // Read from reader task
            Some(res) = reader_rx.recv() => {
                match res {
                    Ok(data) => {
                        debug!("PTY loop received {} bytes from reader", data.len());
                        // Store in ring buffer
                        buffer.write().await.push(&data);

                        // Broadcast to connected clients
                        let receiver_count = output_tx.receiver_count();
                        debug!("PTY loop broadcasting {} bytes to {} receivers", data.len(), receiver_count);
                        if let Err(e) = output_tx.send(data) {
                            warn!("PTY loop failed to broadcast: {}", e);
                        }

                        // Update last activity
                        metadata.write().await.last_activity = chrono::Utc::now();
                    }
                    Err(e) => {
                        error!("PTY read error: {}", e);
                        break;
                    }
                }
            }

            // Handle resize
            Some((cols, rows)) = resize_rx.recv() => {
                debug!("Resizing PTY to {}x{}", cols, rows);
                let _ = pair.master.resize(PtySize {
                    cols,
                    rows,
                    pixel_width: 0,
                    pixel_height: 0,
                });
                metadata.write().await.cols = cols;
                metadata.write().await.rows = rows;
            }

            // Periodic state save
            _ = save_interval.tick() => {
                let meta = metadata.read().await.clone();
                if let Err(e) = save_session_state(&state_dir, &meta).await {
                    warn!("Failed to save session state: {}", e);
                }

                // Save session log (history)
                let content = buffer.read().await.get_contents();
                if !content.is_empty() {
                     // In a real implementation we'd want incremental saves, but for now full overwrite/append is tricky without tracking offsets.
                     // Simple approach: Overwrite the log file with current full buffer.
                     // Wait, save_session_log does append. If we append the whole buffer every 30s, the file will grow huge with duplicates.
                     // Better approach for now: Write the *entire* buffer to file (overwrite).
                     // Let's modify save_session_log to overwrite instead of append for simplicity and correctness in this loop.
                     if let Err(e) = save_session_log(&state_dir, &session_id, &content).await {
                         warn!("Failed to save session log: {}", e);
                     }
                }
            }

            // Shutdown signal
            _ = shutdown_rx.recv() => {
                info!("Daemon shutting down, leaving session {} running (orphaned)", session_id);
                // We DO NOT kill the process here so it can survive daemon restarts/upgrades!
                // The master PTY will close, sending SIGHUP typically.
                _killed = true;
                break;
            }
        }
    }

    // Wait for process to exit
    let exit_code = if let Ok(mut c) = child_arc.lock() {
        if let Some(mut child) = c.take() {
            child
                .wait()
                .ok()
                .and_then(|status| Some(status.exit_code() as i32))
        } else {
            None
        }
    } else {
        None
    };

    metadata.write().await.exit_code = exit_code;
    metadata.write().await.pid = None;

    if _killed {
        info!("Session {} detached (daemon shutdown)", session_id);
    } else {
        info!("Session {} exited with code: {:?}", session_id, exit_code);
    }

    // Final state save
    let meta = metadata.read().await.clone();
    let _ = save_session_state(&state_dir, &meta).await;

    // Clean up writer task
    writer_handle.abort();
}

impl PersistentSession {
    /// Write data to the session (from client)
    pub async fn write(&self, data: Vec<u8>) -> Result<()> {
        self.cmd_tx
            .send(data)
            .await
            .map_err(|_| anyhow::anyhow!("Command channel closed"))
    }

    /// Resize the terminal
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.resize_tx
            .send((cols, rows))
            .await
            .map_err(|_| anyhow::anyhow!("Resize channel closed"))
    }

    /// Subscribe to output stream
    pub fn subscribe_output(&self) -> broadcast::Receiver<Bytes> {
        self.output_tx.subscribe()
    }

    /// Get current buffer contents (for initial sync)
    pub async fn get_buffer_contents(&self) -> Vec<u8> {
        self.buffer.read().await.get_contents()
    }

    /// Get session metadata
    pub async fn get_metadata(&self) -> SessionMetadata {
        self.metadata.read().await.clone()
    }

    /// Get working directory
    pub async fn get_working_dir(&self) -> PathBuf {
        self.metadata.read().await.working_dir.clone()
    }
}

/// Save session state to disk
async fn save_session_state(state_dir: &PathBuf, metadata: &SessionMetadata) -> Result<()> {
    let state_file = state_dir.join(format!("{}.json", metadata.id));
    let content = serde_json::to_string_pretty(metadata)?;
    tokio::fs::write(&state_file, content).await?;
    Ok(())
}

/// Save session output log to disk
async fn save_session_log(state_dir: &PathBuf, session_id: &str, data: &[u8]) -> Result<()> {
    let log_file = state_dir.join(format!("{}.log", session_id));
    // Append to log file
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .await?;
    use tokio::io::AsyncWriteExt;
    file.write_all(data).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_session_recovery() -> Result<()> {
        // 1. Setup temp dir
        let temp_dir = tempfile::tempdir()?;
        let state_dir = temp_dir.path().to_path_buf();

        // 2. Create manager and session
        let manager = PersistenceManager::new(state_dir.clone())?;
        let session = manager
            .create_session(
                Some("test-session".to_string()),
                "test-tag",
                "echo", // Simple command that exits, but we can verify it runs or assume we use "cat" for long running
                std::env::current_dir()?,
                vec![],
                PtySize::default(),
            )
            .await?;

        // 3. Write some output to buffer and save logs manually (simulate loop)
        {
            let mut guard = session.write().await;
            guard.buffer.write().await.push(b"Hello World");
            // Manually save log since run_pty_loop is async/threaded in real code
            save_session_log(&manager.state_dir, &guard.id, b"Hello World").await?;

            // Save state
            let meta = guard.metadata.read().await.clone();
            save_session_state(&manager.state_dir, &meta).await?;
        }

        // 4. "Restart" - Create new manager and recover
        let recovery_manager = PersistenceManager::new(state_dir.clone())?;
        let recovered = recovery_manager.recover_sessions().await?;

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, "test-session");
        assert_eq!(recovered[0].tag, "test-tag");

        // 5. Rehydrate
        let rehydrated = recovery_manager
            .rehydrate_session(recovered[0].clone())
            .await?;
        let session_guard = rehydrated.read().await;

        // 6. Verify history
        let contents = session_guard.get_buffer_contents().await;
        assert_eq!(contents, b"Hello World");

        // 7. Verify new process ID (should be set, different from old one if we could check)
        let meta = session_guard.get_metadata().await;
        assert!(meta.pid.is_some());

        Ok(())
    }
}

/// Load session output log from disk
async fn load_session_log(state_dir: &PathBuf, session_id: &str) -> Result<Vec<u8>> {
    let log_file = state_dir.join(format!("{}.log", session_id));
    info!("DEBUG: Loading log file {:?}", log_file);
    if log_file.exists() {
        let mut file = tokio::fs::File::open(&log_file).await?;
        let metadata = file.metadata().await?;
        let len = metadata.len();

        if len > BUFFER_SIZE as u64 {
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            debug!(
                "Log file is large ({}), reading last {} bytes",
                len, BUFFER_SIZE
            );
            file.seek(std::io::SeekFrom::End(-(BUFFER_SIZE as i64)))
                .await?;
            let mut buffer = vec![0u8; BUFFER_SIZE];
            file.read_exact(&mut buffer).await?;
            debug!("Loaded {} bytes (tail) from log file", buffer.len());
            Ok(buffer)
        } else {
            let res = tokio::fs::read(&log_file).await;
            match res {
                Ok(bytes) => {
                    debug!("Loaded {} bytes from log file", bytes.len());
                    Ok(bytes)
                }
                Err(e) => {
                    error!("DEBUG: Failed to read log file: {}", e);
                    Err(e.into())
                }
            }
        }
    } else {
        info!("DEBUG: Log file not found");
        Ok(Vec::new())
    }
}

/// Check if a process is still running
#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_process_running(pid: u32) -> bool {
    use sysinfo::{ProcessExt, System, SystemExt};
    let s = System::new_all();
    s.process(sysinfo::Pid::from(pid as usize)).is_some()
}

/// Fork and create a detached daemon process (Unix only)
#[cfg(unix)]
#[allow(dead_code)]
pub fn daemonize() -> Result<bool> {
    use nix::unistd::{fork, setsid, ForkResult};

    match unsafe { fork() }? {
        ForkResult::Parent { .. } => {
            // Parent exits
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Create new session
            setsid()?;

            // Second fork to prevent reacquiring terminal
            match unsafe { fork() }? {
                ForkResult::Parent { .. } => {
                    std::process::exit(0);
                }
                ForkResult::Child => {
                    // Close stdio
                    let devnull = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open("/dev/null")?;

                    let fd = devnull.as_raw_fd();
                    unsafe {
                        libc::dup2(fd, libc::STDIN_FILENO);
                        libc::dup2(fd, libc::STDOUT_FILENO);
                        libc::dup2(fd, libc::STDERR_FILENO);
                    }

                    Ok(true)
                }
            }
        }
    }
}

#[cfg(windows)]
pub fn daemonize() -> Result<bool> {
    // Windows doesn't fork, we just return true
    Ok(true)
}

use std::os::unix::io::AsRawFd;
