//! Metrics collection for dashboard visibility
//!
//! Tracks session health, resource usage, and operational metrics

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

/// Global metrics collector
#[allow(dead_code)]
pub struct MetricsCollector {
    /// Per-session metrics
    session_metrics: Arc<RwLock<HashMap<String, SessionMetrics>>>,
    /// Global counters
    counters: Arc<RwLock<GlobalCounters>>,
    /// Start time
    start_time: Instant,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct SessionMetrics {
    pub session_id: String,
    pub tag: String,
    pub status: SessionStatus,
    /// Bytes received from PTY
    pub bytes_in: u64,
    /// Bytes sent to PTY
    pub bytes_out: u64,
    /// Number of connected clients
    pub client_count: usize,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Last activity timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Lines of output produced
    pub output_lines: u64,
    /// Number of errors encountered
    pub error_count: u64,
    /// Current CPU usage (if available)
    pub cpu_percent: Option<f32>,
    /// Current memory usage (if available)
    pub memory_mb: Option<u64>,
    /// Whether session needs confirmation
    pub needs_confirmation: bool,
    /// Current prompt/question text
    pub current_prompt: Option<String>,
    /// Progress percentage (0-100)
    pub progress_percent: Option<u8>,
    /// Current operation description
    pub current_operation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum SessionStatus {
    Initializing,
    Running,
    Idle,
    Error,
    Exited,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct GlobalCounters {
    pub sessions_created: u64,
    pub sessions_closed: u64,
    pub total_connections: u64,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub errors_total: u64,
    pub confirmations_requested: u64,
    pub confirmations_resolved: u64,
}

impl Default for GlobalCounters {
    fn default() -> Self {
        Self {
            sessions_created: 0,
            sessions_closed: 0,
            total_connections: 0,
            total_bytes_in: 0,
            total_bytes_out: 0,
            errors_total: 0,
            confirmations_requested: 0,
            confirmations_resolved: 0,
        }
    }
}

/// Dashboard summary for all sessions
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct DashboardSummary {
    /// Total active sessions
    pub total_sessions: usize,
    /// Sessions with active clients
    pub connected_sessions: usize,
    /// Sessions needing user attention
    pub sessions_needing_attention: usize,
    /// Sessions with pending confirmations
    pub pending_confirmations: usize,
    /// Sessions currently executing tasks
    pub active_tasks: usize,
    /// Global error count
    pub global_errors: u64,
    /// Daemon uptime seconds
    pub uptime_secs: u64,
    /// Per-session summaries
    pub sessions: Vec<SessionMetrics>,
}

impl MetricsCollector {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            session_metrics: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(GlobalCounters::default())),
            start_time: Instant::now(),
        }
    }

    /// Initialize or update session metrics
    #[allow(dead_code)]
    pub async fn register_session(&self, session_id: &str, tag: &str) {
        let mut metrics = self.session_metrics.write().await;
        metrics.insert(
            session_id.to_string(),
            SessionMetrics {
                session_id: session_id.to_string(),
                tag: tag.to_string(),
                status: SessionStatus::Initializing,
                bytes_in: 0,
                bytes_out: 0,
                client_count: 0,
                uptime_secs: 0,
                last_activity: chrono::Utc::now(),
                output_lines: 0,
                error_count: 0,
                cpu_percent: None,
                memory_mb: None,
                needs_confirmation: false,
                current_prompt: None,
                progress_percent: None,
                current_operation: None,
            },
        );

        let mut counters = self.counters.write().await;
        counters.sessions_created += 1;
        debug!("Registered session {} with tag {}", session_id, tag);
    }

    /// Update session status
    #[allow(dead_code)]
    pub async fn update_session_status(&self, session_id: &str, status: SessionStatus) {
        let mut metrics = self.session_metrics.write().await;
        if let Some(m) = metrics.get_mut(session_id) {
            m.status = status;
            m.last_activity = chrono::Utc::now();
        }
    }

    /// Update session I/O metrics
    #[allow(dead_code)]
    pub async fn record_io(&self, session_id: &str, bytes_in: u64, bytes_out: u64) {
        let mut metrics = self.session_metrics.write().await;
        if let Some(m) = metrics.get_mut(session_id) {
            m.bytes_in += bytes_in;
            m.bytes_out += bytes_out;
            m.last_activity = chrono::Utc::now();
        }

        let mut counters = self.counters.write().await;
        counters.total_bytes_in += bytes_in;
        counters.total_bytes_out += bytes_out;
    }

    /// Update client count
    #[allow(dead_code)]
    pub async fn update_client_count(&self, session_id: &str, count: usize) {
        let mut metrics = self.session_metrics.write().await;
        if let Some(m) = metrics.get_mut(session_id) {
            m.client_count = count;
            m.last_activity = chrono::Utc::now();
        }
    }

    /// Set confirmation state
    #[allow(dead_code)]
    pub async fn set_confirmation_state(
        &self,
        session_id: &str,
        needs_confirmation: bool,
        prompt: Option<String>,
    ) {
        let mut metrics = self.session_metrics.write().await;
        if let Some(m) = metrics.get_mut(session_id) {
            m.needs_confirmation = needs_confirmation;
            m.current_prompt = prompt;
            m.last_activity = chrono::Utc::now();

            if needs_confirmation {
                m.status = SessionStatus::Idle;
            }
        }
    }

    /// Set progress information
    #[allow(dead_code)]
    pub async fn set_progress(&self, session_id: &str, percent: u8, operation: String) {
        let mut metrics = self.session_metrics.write().await;
        if let Some(m) = metrics.get_mut(session_id) {
            m.progress_percent = Some(percent.min(100));
            m.current_operation = Some(operation);
            m.status = SessionStatus::Running;
            m.last_activity = chrono::Utc::now();
        }
    }

    /// Record error for session
    #[allow(dead_code)]
    pub async fn record_error(&self, session_id: &str, _error: &str) {
        let mut metrics = self.session_metrics.write().await;
        if let Some(m) = metrics.get_mut(session_id) {
            m.error_count += 1;
            m.status = SessionStatus::Error;
            m.last_activity = chrono::Utc::now();
        }

        let mut counters = self.counters.write().await;
        counters.errors_total += 1;
    }

    /// Remove session metrics
    #[allow(dead_code)]
    pub async fn remove_session(&self, session_id: &str) {
        self.session_metrics.write().await.remove(session_id);
        let mut counters = self.counters.write().await;
        counters.sessions_closed += 1;
    }

    /// Get dashboard summary
    #[allow(dead_code)]
    pub async fn get_dashboard_summary(&self) -> DashboardSummary {
        let metrics = self.session_metrics.read().await;
        let counters = self.counters.read().await;

        let sessions: Vec<_> = metrics.values().cloned().collect();
        let connected_sessions = sessions.iter().filter(|s| s.client_count > 0).count();
        let sessions_needing_attention = sessions
            .iter()
            .filter(|s| s.needs_confirmation || s.error_count > 0)
            .count();
        let pending_confirmations = sessions.iter().filter(|s| s.needs_confirmation).count();
        let active_tasks = sessions
            .iter()
            .filter(|s| s.progress_percent.is_some() && matches!(s.status, SessionStatus::Running))
            .count();

        DashboardSummary {
            total_sessions: sessions.len(),
            connected_sessions,
            sessions_needing_attention,
            pending_confirmations,
            active_tasks,
            global_errors: counters.errors_total,
            uptime_secs: self.start_time.elapsed().as_secs(),
            sessions,
        }
    }

    /// Get session metrics
    #[allow(dead_code)]
    pub async fn get_session_metrics(&self, session_id: &str) -> Option<SessionMetrics> {
        self.session_metrics.read().await.get(session_id).cloned()
    }

    /// Get global counters
    #[allow(dead_code)]
    pub async fn get_counters(&self) -> GlobalCounters {
        self.counters.read().await.clone()
    }

    /// Periodic cleanup and maintenance
    #[allow(dead_code)]
    pub async fn maintenance(&self) {
        let mut metrics = self.session_metrics.write().await;
        for m in metrics.values_mut() {
            // Update uptime
            // Note: In production, you'd track session start time more accurately
            m.uptime_secs += 1;
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Background metrics tasks
#[allow(dead_code)]
pub async fn start_metrics_background_tasks(collector: Arc<MetricsCollector>) {
    // Maintenance task
    let collector_clone = collector.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            collector_clone.maintenance().await;
        }
    });
}
