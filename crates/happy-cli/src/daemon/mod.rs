//! Daemon management

use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;

pub mod bridge;
pub mod error;
pub mod metrics;
pub mod multiplexer;
pub mod persistence;
pub mod rpc;
pub mod rpc_server;
pub mod server;
pub mod session_manager;

pub struct DaemonManager;

impl DaemonManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn is_running(&self) -> bool {
        if let Ok(pid_path) = crate::config::SettingsManager::pid_path() {
            if let Ok(pid_str) = tokio::fs::read_to_string(&pid_path).await {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    // Check if process exists
                    #[cfg(unix)]
                    {
                        return unsafe { libc::kill(pid as i32, 0) == 0 };
                    }
                    #[cfg(windows)]
                    {
                        use sysinfo::{ProcessExt, System, SystemExt};
                        let s = System::new_all();
                        return s.process(sysinfo::Pid::from(pid as usize)).is_some();
                    }
                }
            }
        }
        false
    }

    pub async fn start(&self) -> Result<()> {
        let happy_home = crate::config::SettingsManager::happy_home()?;
        let pid_path = crate::config::SettingsManager::pid_path()?;

        // Ensure directory exists
        tokio::fs::create_dir_all(&happy_home).await?;

        // Get the current executable path
        let current_exe = std::env::current_exe()?;

        // Spawn daemon process
        let mut cmd = Command::new(current_exe);
        cmd.arg("daemon")
            .arg("run")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir(&happy_home)
            .env("HAPPY_DAEMON", "1");

        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        let child = cmd.spawn().context("Failed to spawn daemon process")?;

        // Write PID file
        let pid = child.id().context("Failed to get daemon PID")?;
        tokio::fs::write(&pid_path, pid.to_string()).await?;

        // Wait a moment to ensure daemon started
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let pid_path = crate::config::SettingsManager::pid_path()?;

        if let Ok(pid_str) = tokio::fs::read_to_string(&pid_path).await {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                #[cfg(unix)]
                {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
                #[cfg(windows)]
                {
                    use sysinfo::{ProcessExt, System, SystemExt};
                    let mut s = System::new_all();
                    s.refresh_all();
                    if let Some(process) = s.process(sysinfo::Pid::from(pid as usize)) {
                        process.kill();
                    }
                }
            }
        }

        // Remove PID file
        let _ = tokio::fs::remove_file(&pid_path).await;

        Ok(())
    }
}

/// Client for communicating with the daemon
pub struct DaemonClient {
    rpc_port: u16,
}

impl DaemonClient {
    pub async fn connect() -> Result<Self> {
        Ok(Self {
            rpc_port: 16792, // TODO: Get from settings
        })
    }

    /// Start a new session on the server via Daemon RPC
    pub async fn start_session(&self, id: Option<String>, tag: &str, cwd: &str) -> Result<SessionInfo> {
        // We need to resolve the token locally first to send it to Daemon
        // Or should Daemon resolve it?
        // The Daemon might be running as a different user (system service)? No, usually same user.
        // But the Daemon needs the token to Connect to Server.
        // Let's pass the token from CLI to Daemon.

        let settings = crate::config::SettingsManager::load().context("Failed to load settings")?;

        let token = settings
            .access_token
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        // Also need server URL
        let server_url = settings.server_url; // Assuming this field exists or similar

        let request = rpc::DaemonRequest::StartSession {
            id,
            tag: tag.to_string(),
            token,
            server_url,
            cwd: cwd.to_string(),
        };

        match self.send_rpc(request).await? {
            rpc::DaemonResponse::SessionStarted { session_id } => {
                Ok(SessionInfo { id: session_id })
            }
            rpc::DaemonResponse::Error(e) => anyhow::bail!("Daemon error: {}", e),
            _ => anyhow::bail!("Unexpected response from daemon"),
        }
    }

    async fn send_rpc(&self, request: rpc::DaemonRequest) -> Result<rpc::DaemonResponse> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let addr = format!("127.0.0.1:{}", self.rpc_port);
        let mut stream = TcpStream::connect(addr)
            .await
            .context("Failed to connect to daemon RPC port")?;

        let req_bytes = serde_json::to_vec(&request)?;
        stream.write_all(&req_bytes).await?;
        stream.shutdown().await?; // Close write side to signal end of request?
                                  // Or just rely on one-shot connection.
                                  // My server reads `read_to_end`, so closing write side IS required if using that.

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        let response: rpc::DaemonResponse = serde_json::from_slice(&buf)?;
        Ok(response)
    }

    pub async fn attach_session(&self, session_id: &str) -> Result<()> {
        use futures::{SinkExt, StreamExt};
        use nix::sys::termios::{self, SetArg};
        use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio_tungstenite::tungstenite::Message;

        let port = crate::commands::config::get_daemon_port().await;
        let ws_url = format!("ws://127.0.0.1:{}", port);

        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .context("Failed to connect to daemon WebSocket")?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let attach_msg = crate::daemon::server::ClientMessage::AttachSession {
            session_id: session_id.to_string(),
        };
        ws_tx
            .send(Message::Text(serde_json::to_string(&attach_msg)?))
            .await?;

        let stdin_handle = std::io::stdin();
        let stdin_fd = stdin_handle.as_fd();
        let original_termios = termios::tcgetattr(stdin_fd)?;
        let mut raw_termios = original_termios.clone();
        termios::cfmakeraw(&mut raw_termios);
        termios::tcsetattr(stdin_fd, SetArg::TCSANOW, &raw_termios)?;
        struct TermiosGuard {
            fd: i32,
            original: termios::Termios,
        }
        impl Drop for TermiosGuard {
            fn drop(&mut self) {
                unsafe {
                    let fd = BorrowedFd::borrow_raw(self.fd);
                    let _ = termios::tcsetattr(fd, SetArg::TCSANOW, &self.original);
                }
            }
        }
        let _guard = TermiosGuard {
            fd: stdin_handle.as_raw_fd(),
            original: original_termios,
        };

        // Track consecutive Ctrl+C for exit detection
        let mut last_ctrl_c_time: Option<std::time::Instant> = None;
        let ctrl_c_timeout = std::time::Duration::from_secs(1);

        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut input_buf = [0u8; 4096];

        loop {
            tokio::select! {
                msg = ws_rx.next() => {
                    let msg = match msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                            tracing::error!("WebSocket receive error: {}", e);
                            break;
                        }
                        None => {
                            tracing::info!("WebSocket stream ended");
                            break;
                        }
                    };
                    match msg {
                        Message::Text(text) => {
                            if let Ok(server_msg) = serde_json::from_str::<crate::daemon::server::ServerMessage>(&text) {
                                match server_msg {
                                    crate::daemon::server::ServerMessage::SessionAttached { buffer, .. } => {
                                        tracing::info!("Session attached, buffer len: {}", buffer.len());
                                        if !buffer.is_empty() {
                                            stdout.write_all(buffer.as_bytes()).await?;
                                            stdout.flush().await?;
                                        }
                                    }
                                    crate::daemon::server::ServerMessage::Output { data } => {
                                        stdout.write_all(data.as_bytes()).await?;
                                        stdout.flush().await?;
                                    }
                                    crate::daemon::server::ServerMessage::Error { message } => {
                                        eprintln!("Error: {}", message);
                                    }
                                    crate::daemon::server::ServerMessage::SessionDetached { .. } => {
                                        tracing::info!("Session detached");
                                        break;
                                    }
                                    crate::daemon::server::ServerMessage::SessionKilled { .. } => {
                                        tracing::info!("Session killed");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Message::Binary(data) => {
                            stdout.write_all(&data).await?;
                            stdout.flush().await?;
                        }
                        Message::Close(_) => {
                            tracing::info!("WebSocket close received");
                            break;
                        }
                        _ => {}
                    }
                }
                read = stdin.read(&mut input_buf) => {
                    match read {
                        Ok(n) => {
                            if n == 0 {
                                tracing::info!("stdin EOF");
                                break;
                            }

                            // Check for Ctrl+C (0x03) in raw mode for double-press exit
                            let data = &input_buf[..n];
                            let now = std::time::Instant::now();

                            // Detect Ctrl+C: single byte 0x03
                            if data.len() == 1 && data[0] == 0x03 {
                                if let Some(last_time) = last_ctrl_c_time {
                                    if now.duration_since(last_time) < ctrl_c_timeout {
                                        // Double Ctrl+C - exit
                                        tracing::info!("Double Ctrl+C detected, detaching...");
                                        println!("\r\nDetaching from session...");
                                        break;
                                    }
                                }
                                last_ctrl_c_time = Some(now);
                                // Still forward the Ctrl+C to the PTY so Claude can handle it
                            } else {
                                // Reset Ctrl+C timer on other input
                                last_ctrl_c_time = None;
                            }

                            if let Err(e) = ws_tx.send(Message::Binary(data.to_vec())).await {
                                tracing::error!("Failed to send to WebSocket: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("stdin read error: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        tracing::info!("attach_session loop ended");
        Ok(())
    }

    pub async fn get_info(&self) -> Result<DaemonInfo> {
        // Count active sessions by checking session directories
        let happy_home = crate::config::SettingsManager::happy_home()?;
        let sessions_dir = happy_home.join("sessions");

        let mut active_count = 0;
        if let Ok(mut entries) = tokio::fs::read_dir(&sessions_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "json") {
                    if let Ok(content) = tokio::fs::read_to_string(&path).await {
                        if let Ok(metadata) =
                            serde_json::from_str::<persistence::SessionMetadata>(&content)
                        {
                            if let Some(pid) = metadata.pid {
                                #[cfg(unix)]
                                {
                                    if unsafe { libc::kill(pid as i32, 0) } == 0 {
                                        active_count += 1;
                                    }
                                }
                                #[cfg(windows)]
                                {
                                    use sysinfo::{ProcessExt, System, SystemExt};
                                    let s = System::new_all();
                                    if s.process(sysinfo::Pid::from(pid as usize)).is_some() {
                                        active_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(DaemonInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: 0, // TODO: Track actual uptime
            active_sessions: active_count,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct DaemonInfo {
    pub version: String,
    pub uptime_secs: u64,
    pub active_sessions: usize,
}
