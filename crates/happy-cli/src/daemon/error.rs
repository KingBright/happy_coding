//! Error handling and recovery for daemon
//!
//! Implements circuit breaker pattern and automatic recovery strategies

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Maximum number of consecutive failures before opening circuit
#[allow(dead_code)]
const FAILURE_THRESHOLD: u32 = 5;

/// Duration to keep circuit open before trying again
#[allow(dead_code)]
const RESET_TIMEOUT: Duration = Duration::from_secs(60);

/// Retry backoff base duration
#[allow(dead_code)]
const RETRY_BASE_MS: u64 = 100;

/// Maximum retry attempts
#[allow(dead_code)]
const MAX_RETRIES: u32 = 3;

/// Circuit breaker states
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation
    Closed,
    /// Open - requests fail immediately
    Open,
    /// Half-open - testing if service recovered
    HalfOpen,
}

/// Error context for dashboard visibility
#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct ErrorContext {
    /// Error code for programmatic handling
    pub code: String,
    /// Human readable message
    pub message: String,
    /// Severity level
    pub severity: ErrorSeverity,
    /// Associated session ID (if any)
    pub session_id: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Suggested recovery action
    pub recovery_action: Option<String>,
    /// Whether this error requires user confirmation
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Circuit breaker for external service calls
#[allow(dead_code)]
pub struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_count: AtomicU32,
    last_failure_time: RwLock<Option<Instant>>,
    name: String,
}

impl CircuitBreaker {
    #[allow(dead_code)]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            last_failure_time: RwLock::new(None),
            name: name.into(),
        }
    }

    /// Check if request can proceed
    #[allow(dead_code)]
    pub async fn can_execute(&self) -> Result<()> {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if we should transition to half-open
                let last_failure = *self.last_failure_time.read().await;
                if let Some(time) = last_failure {
                    if time.elapsed() > RESET_TIMEOUT {
                        info!("Circuit breaker '{}' transitioning to half-open", self.name);
                        *self.state.write().await = CircuitState::HalfOpen;
                        return Ok(());
                    }
                }

                Err(anyhow::anyhow!(
                    "Circuit breaker '{}' is open - service unavailable",
                    self.name
                ))
            }
            CircuitState::HalfOpen => Ok(()),
        }
    }

    /// Record a success
    #[allow(dead_code)]
    pub async fn record_success(&self) {
        let state = *self.state.read().await;

        if state == CircuitState::HalfOpen {
            info!("Circuit breaker '{}' closing", self.name);
            *self.state.write().await = CircuitState::Closed;
        }

        self.failure_count.store(0, Ordering::Relaxed);
    }

    /// Record a failure
    #[allow(dead_code)]
    pub async fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.write().await = Some(Instant::now());

        if count >= FAILURE_THRESHOLD {
            warn!(
                "Circuit breaker '{}' opening after {} failures",
                self.name, count
            );
            *self.state.write().await = CircuitState::Open;
        }
    }

    /// Get current state
    #[allow(dead_code)]
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }
}

/// Retry executor with exponential backoff
#[allow(dead_code)]
pub struct RetryExecutor {
    max_retries: u32,
    base_delay: Duration,
}

impl Default for RetryExecutor {
    fn default() -> Self {
        Self {
            max_retries: MAX_RETRIES,
            base_delay: Duration::from_millis(RETRY_BASE_MS),
        }
    }
}

impl RetryExecutor {
    #[allow(dead_code)]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Execute operation with retry
    #[allow(dead_code)]
    pub async fn execute<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        let delay = self.base_delay * 2_u32.pow(attempt);
                        warn!(
                            "Operation failed (attempt {}/{}), retrying in {:?}...",
                            attempt + 1,
                            self.max_retries + 1,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Operation failed after {} attempts: {:?}",
            self.max_retries + 1,
            last_error
        ))
    }
}

/// Error manager for dashboard visibility
#[allow(dead_code)]
pub struct ErrorManager {
    errors: Arc<RwLock<Vec<ErrorContext>>>,
    max_errors: usize,
}

impl ErrorManager {
    #[allow(dead_code)]
    pub fn new(max_errors: usize) -> Self {
        Self {
            errors: Arc::new(RwLock::new(Vec::new())),
            max_errors,
        }
    }

    /// Record an error
    #[allow(dead_code)]
    pub async fn report_error(&self, error: ErrorContext) {
        let mut errors = self.errors.write().await;

        if error.severity == ErrorSeverity::Critical {
            error!("CRITICAL ERROR [{}]: {}", error.code, error.message);
        } else {
            warn!("Error [{}]: {}", error.code, error.message);
        }

        errors.push(error);

        // Trim old errors
        while errors.len() > self.max_errors {
            errors.remove(0);
        }
    }

    /// Get all errors
    #[allow(dead_code)]
    pub async fn get_errors(&self) -> Vec<ErrorContext> {
        self.errors.read().await.clone()
    }

    /// Get errors requiring confirmation
    #[allow(dead_code)]
    pub async fn get_pending_confirmations(&self) -> Vec<ErrorContext> {
        self.errors
            .read()
            .await
            .iter()
            .filter(|e| e.requires_confirmation)
            .cloned()
            .collect()
    }

    /// Clear errors for a session
    #[allow(dead_code)]
    pub async fn clear_session_errors(&self, session_id: &str) {
        self.errors
            .write()
            .await
            .retain(|e| e.session_id.as_deref() != Some(session_id));
    }

    /// Get recent errors summary
    #[allow(dead_code)]
    pub async fn get_error_summary(&self) -> ErrorSummary {
        let errors = self.errors.read().await;

        ErrorSummary {
            total: errors.len(),
            critical: errors
                .iter()
                .filter(|e| matches!(e.severity, ErrorSeverity::Critical))
                .count(),
            pending_confirmations: errors.iter().filter(|e| e.requires_confirmation).count(),
            last_error_time: errors.last().map(|e| e.timestamp),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct ErrorSummary {
    pub total: usize,
    pub critical: usize,
    pub pending_confirmations: usize,
    pub last_error_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// Session recovery strategies
#[allow(dead_code)]
pub enum RecoveryStrategy {
    /// Restart the session
    Restart,
    /// Kill and recreate
    Recreate,
    /// Wait for manual intervention
    ManualIntervention,
    /// Ignore and continue
    Ignore,
}

/// Auto-recovery executor
#[allow(dead_code)]
pub struct RecoveryExecutor {
    error_manager: Arc<ErrorManager>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl RecoveryExecutor {
    #[allow(dead_code)]
    pub fn new(error_manager: Arc<ErrorManager>, circuit_breaker: Arc<CircuitBreaker>) -> Self {
        Self {
            error_manager,
            circuit_breaker,
        }
    }

    /// Execute operation with full error handling
    #[allow(dead_code)]
    pub async fn execute_with_recovery<F, Fut, T>(
        &self,
        operation: F,
        session_id: Option<String>,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check circuit breaker
        if let Err(e) = self.circuit_breaker.can_execute().await {
            self.error_manager
                .report_error(ErrorContext {
                    code: "CIRCUIT_OPEN".into(),
                    message: e.to_string(),
                    severity: ErrorSeverity::Warning,
                    session_id: session_id.clone(),
                    timestamp: chrono::Utc::now(),
                    recovery_action: Some("Wait for circuit to close".into()),
                    requires_confirmation: false,
                })
                .await;
            return Err(e);
        }

        // Execute with retry
        let retry = RetryExecutor::default();
        match retry.execute(operation).await {
            Ok(result) => {
                self.circuit_breaker.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.circuit_breaker.record_failure().await;

                let error = ErrorContext {
                    code: "OPERATION_FAILED".into(),
                    message: e.to_string(),
                    severity: ErrorSeverity::Error,
                    session_id,
                    timestamp: chrono::Utc::now(),
                    recovery_action: Some("Check logs and retry".into()),
                    requires_confirmation: true,
                };
                self.error_manager.report_error(error).await;

                Err(e)
            }
        }
    }
}
