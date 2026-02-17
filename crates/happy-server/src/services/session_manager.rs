//! Session management service

use crate::storage::{Database, MemoryCache};
use anyhow::Result;
use happy_core::{Session, SessionStatus};
use std::sync::Arc;
use tracing::{debug, info};

pub struct SessionManager {
    db: Arc<Database>,
    cache: Arc<MemoryCache>,
}

impl SessionManager {
    pub fn new(db: Arc<Database>, cache: Arc<MemoryCache>) -> Self {
        Self { db, cache }
    }

    pub async fn create_session(
        &self,
        user_id: &str,
        machine_id: &str,
        machine_name: &str,
        tag: &str,
        cwd: &str,
    ) -> Result<Session> {
        info!(
            "Creating session: user={}, tag={}, machine={}, cwd={}",
            user_id, tag, machine_name, cwd
        );

        let id = uuid::Uuid::new_v4().to_string();
        let mut session = Session::new(
            id,
            tag.to_string(),
            user_id.to_string(),
            machine_id.to_string(),
            machine_name.to_string(),
        );
        session.metadata.cwd = cwd.to_string();

        // Save to database
        self.db.create_session(&session).await?;

        // Cache active session
        let session_key = format!("session:{}", session.id);
        let session_json = serde_json::to_vec(&session)?;
        self.cache.set(session_key, session_json);

        Ok(session)
    }

    /// Create a session from remote CLI with a specific ID and cwd
    pub async fn create_session_from_remote(
        &self,
        user_id: &str,
        machine_id: &str,
        machine_name: &str,
        tag: &str,
        session_id: &str,
        cwd: &str,
    ) -> Result<Session> {
        info!(
            "Creating remote session: id={}, user={}, tag={}, machine={}, cwd={}",
            session_id, user_id, tag, machine_name, cwd
        );

        let mut session = Session::new(
            session_id.to_string(),
            tag.to_string(),
            user_id.to_string(),
            machine_id.to_string(),
            machine_name.to_string(),
        );
        session.metadata.cwd = cwd.to_string();

        // Save to database
        self.db.create_session(&session).await?;

        // Cache active session
        let session_key = format!("session:{}", session.id);
        let session_json = serde_json::to_vec(&session)?;
        self.cache.set(session_key, session_json);

        Ok(session)
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<Session>> {
        // Try cache first
        let session_key = format!("session:{}", id);
        if let Some(data) = self.cache.get(&session_key) {
            if let Ok(session) = serde_json::from_slice::<Session>(&data) {
                return Ok(Some(session));
            }
        }

        // Fall back to database
        self.db.get_session(id).await
    }

    pub async fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()> {
        debug!("Updating session {} status to {:?}", id, status);

        self.db.update_session_status(id, status.clone()).await?;

        // Update cache if present
        let session_key = format!("session:{}", id);
        if let Some(data) = self.cache.get(&session_key) {
            if let Ok(mut session) = serde_json::from_slice::<Session>(&data) {
                session.status = status;
                let session_json = serde_json::to_vec(&session)?;
                self.cache.set(session_key, session_json);
            }
        }

        Ok(())
    }

    pub async fn update_session_cwd(&self, id: &str, cwd: &str) -> Result<()> {
        debug!("Updating session {} cwd to {}", id, cwd);

        self.db.update_session_cwd(id, cwd).await?;

        // Update cache if present
        let session_key = format!("session:{}", id);
        if let Some(data) = self.cache.get(&session_key) {
            if let Ok(mut session) = serde_json::from_slice::<Session>(&data) {
                session.metadata.cwd = cwd.to_string();
                let session_json = serde_json::to_vec(&session)?;
                self.cache.set(session_key, session_json);
            }
        }

        Ok(())
    }

    pub async fn update_session_machine(
        &self,
        id: &str,
        machine_id: &str,
        machine_name: &str,
    ) -> Result<()> {
        debug!(
            "Updating session {} machine to {} ({})",
            id, machine_name, machine_id
        );

        self.db
            .update_session_machine(id, machine_id, machine_name)
            .await?;

        // Update cache if present
        let session_key = format!("session:{}", id);
        if let Some(data) = self.cache.get(&session_key) {
            if let Ok(mut session) = serde_json::from_slice::<Session>(&data) {
                session.machine_id = machine_id.to_string();
                session.machine_name = machine_name.to_string();
                let session_json = serde_json::to_vec(&session)?;
                self.cache.set(session_key, session_json);
            }
        }

        Ok(())
    }

    pub async fn list_user_sessions(&self, user_id: &str) -> Result<Vec<Session>> {
        self.db.list_sessions_by_user(user_id).await
    }

    pub async fn find_session_by_tag(&self, user_id: &str, tag: &str) -> Result<Option<Session>> {
        let sessions = self.db.list_sessions_by_user(user_id).await?;
        // Include Initializing, Running, and Paused sessions
        Ok(sessions
            .into_iter()
            .find(|s| s.tag == tag && !matches!(s.status, SessionStatus::Terminated)))
    }

    pub async fn list_active_machine_sessions(&self, machine_id: &str) -> Result<Vec<Session>> {
        self.db.list_active_sessions_by_machine(machine_id).await
    }

    pub async fn terminate_session(&self, id: &str) -> Result<()> {
        info!("Terminating session: {}", id);

        self.update_session_status(id, SessionStatus::Terminated)
            .await?;

        // Remove from cache
        let session_key = format!("session:{}", id);
        self.cache.delete(&session_key);

        Ok(())
    }

    pub async fn remove_session(&self, id: &str) -> Result<()> {
        info!("Removing session: {}", id);

        // Delete from database
        self.db.delete_session(id).await?;

        // Remove from cache
        let session_key = format!("session:{}", id);
        self.cache.delete(&session_key);

        Ok(())
    }
}
