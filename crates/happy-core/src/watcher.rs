//! File watcher for development mode

use crate::error::{HappyError, Result};
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// Events emitted by the file watcher
#[derive(Debug, Clone)]
pub enum WatchEvent {
    Changed(PathBuf),
    Created(PathBuf),
    Removed(PathBuf),
    Error(String),
}

/// File watcher for development mode
pub struct Watcher {
    tx: Option<Sender<WatchEvent>>,
    rx: Option<Receiver<WatchEvent>>,
    debounce_ms: u64,
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Watcher {
    /// Create a new file watcher
    pub fn new() -> Self {
        Self {
            tx: None,
            rx: None,
            debounce_ms: 500,
        }
    }

    /// Set debounce duration in milliseconds
    pub fn with_debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Start watching a directory
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        let (tx, rx) = channel();
        self.tx = Some(tx.clone());
        self.rx = Some(rx);

        let debounce_duration = Duration::from_millis(self.debounce_ms);

        let mut debouncer = new_debouncer(
            debounce_duration,
            move |res: std::result::Result<
                Vec<notify_debouncer_mini::DebouncedEvent>,
                notify::Error,
            >| match res {
                Ok(events) => {
                    for event in events {
                        let watch_event = match event.kind {
                            DebouncedEventKind::Any => WatchEvent::Changed(event.path.clone()),
                            DebouncedEventKind::AnyContinuous => continue,
                            _ => continue,
                        };
                        let _ = tx.send(watch_event);
                    }
                }
                Err(e) => {
                    let _ = tx.send(WatchEvent::Error(e.to_string()));
                }
            },
        )
        .map_err(|e| HappyError::Watch(e.to_string()))?;

        debouncer
            .watcher()
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| HappyError::Watch(e.to_string()))?;

        // Store the debouncer to keep it alive
        // In a real implementation, we'd store this in the struct
        std::mem::forget(debouncer);

        Ok(())
    }

    /// Get the next event (blocking)
    pub fn next_event(&self) -> Option<WatchEvent> {
        self.rx.as_ref().and_then(|rx| rx.recv().ok())
    }

    /// Try to get the next event (non-blocking)
    pub fn try_next_event(&self) -> Option<WatchEvent> {
        self.rx.as_ref().and_then(|rx| rx.try_recv().ok())
    }

    /// Check if there are pending events
    pub fn has_pending(&self) -> bool {
        self.rx
            .as_ref()
            .map(|rx| !rx.try_iter().peekable().peek().is_none())
            .unwrap_or(false)
    }
}

/// Filter for config file changes
pub fn is_config_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            name == "happy.config.yaml"
                || name == "happy.config.yml"
                || name == "happy.config.json"
        })
        .unwrap_or(false)
}

/// Filter for skill/workflow files
pub fn is_source_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str());
    matches!(ext, Some("yaml") | Some("yml") | Some("md") | Some("json"))
}
