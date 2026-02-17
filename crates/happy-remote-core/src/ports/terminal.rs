//! Terminal PTY abstraction traits

use crate::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::path::Path;

/// Terminal backend for spawning PTY sessions
#[async_trait]
pub trait TerminalBackend: Send + Sync {
    /// Spawn a new terminal session
    async fn spawn(
        &self,
        shell: &str,
        cwd: &Path,
        env: &HashMap<String, String>,
        cols: u16,
        rows: u16,
    ) -> Result<Box<dyn TerminalSession>>;
}

/// Active terminal session
#[async_trait]
pub trait TerminalSession: Send + Sync {
    /// Resize the terminal
    async fn resize(&mut self, cols: u16, rows: u16) -> Result<()>;

    /// Write data to the terminal
    async fn write(&mut self, data: &[u8]) -> Result<()>;

    /// Read data from the terminal (non-blocking)
    async fn read(&mut self) -> Result<Option<Bytes>>;

    /// Kill the terminal session
    async fn kill(&mut self) -> Result<()>;

    /// Check if the session is still alive
    fn is_alive(&self) -> bool;

    /// Get the process ID
    fn pid(&self) -> u32;

    /// Get the exit code if the process has exited
    fn exit_code(&self) -> Option<i32>;
}
