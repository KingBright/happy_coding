//! SQLite database layer (embedded, no external dependencies)

use anyhow::{Context, Result};
use happy_core::{Machine, Platform, Session, SessionStatus};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::sync::Arc;

pub struct Database {
    pool: Arc<SqlitePool>,
}

impl Database {
    pub async fn new(database_path: &str) -> Result<Self> {
        tracing::info!("Opening SQLite database at: {}", database_path);

        // Create parent directory if needed
        if let Some(parent) = std::path::Path::new(database_path).parent() {
            tracing::info!("Creating parent directory: {}", parent.display());
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create database directory: {}", parent.display())
            })?;
        }

        // Check if directory is writable
        let parent = std::path::Path::new(database_path)
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid database path: no parent directory"))?;

        let test_file = parent.join(".write_test");
        match tokio::fs::write(&test_file, b"test").await {
            Ok(_) => {
                let _ = tokio::fs::remove_file(&test_file).await;
                tracing::info!("Database directory is writable");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Database directory is not writable: {}: {}",
                    parent.display(),
                    e
                ));
            }
        }

        tracing::info!("Connecting to SQLite...");

        // Use SqliteConnectOptions for better control
        let options = SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .with_context(|| {
                format!("Failed to connect to SQLite database at: {}", database_path)
            })?;

        tracing::info!("SQLite connection established, running migrations...");

        // Run migrations (inline for simplicity)
        Self::run_migrations(&pool)
            .await
            .context("Failed to run database migrations")?;

        tracing::info!("Database initialization complete");

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        // Users table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                name TEXT,
                password_hash TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Sessions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                tag TEXT NOT NULL,
                user_id TEXT NOT NULL,
                machine_id TEXT NOT NULL,
                machine_name TEXT DEFAULT 'Unknown',
                status TEXT DEFAULT 'initializing',
                encrypted_data_key BLOB,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_activity DATETIME DEFAULT CURRENT_TIMESTAMP,
                cwd TEXT DEFAULT '/',
                env TEXT DEFAULT '{}',
                claude_version TEXT,
                shell TEXT DEFAULT '/bin/bash'
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Migration: Add machine_name column if it doesn't exist
        let _ = sqlx::query(
            r#"
            ALTER TABLE sessions ADD COLUMN machine_name TEXT DEFAULT 'Unknown'
            "#,
        )
        .execute(pool)
        .await; // Ignore error if column already exists

        // Machines table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS machines (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                public_key BLOB NOT NULL,
                platform TEXT DEFAULT 'linux',
                capabilities TEXT DEFAULT 'terminal,file_system',
                last_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                hostname TEXT
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Access keys table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS access_keys (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                key_prefix TEXT NOT NULL,
                permissions TEXT DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME,
                last_used_at DATETIME
            )
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    // User operations
    pub async fn create_user(
        &self,
        email: &str,
        password_hash: &str,
        name: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO users (id, email, name, password_hash)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&id)
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .execute(&*self.pool)
        .await?;

        Ok(id)
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<(String, String)>> {
        let row: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT id, password_hash FROM users WHERE email = ?1
            "#,
        )
        .bind(email)
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_user_by_id(
        &self,
        user_id: &str,
    ) -> Result<Option<(String, String, Option<String>)>> {
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, email, name FROM users WHERE id = ?1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    // Session operations
    pub async fn create_session(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sessions (id, tag, user_id, machine_id, machine_name, status, cwd, env, shell)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&session.id)
        .bind(&session.tag)
        .bind(&session.user_id)
        .bind(&session.machine_id)
        .bind(&session.machine_name)
        .bind(session.status.to_string())
        .bind(&session.metadata.cwd)
        .bind(serde_json::to_string(&session.metadata.env)?)
        .bind(&session.metadata.shell)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let row: Option<SessionRow> = sqlx::query_as(
            r#"
            SELECT id, tag, user_id, machine_id, machine_name, status,
                   encrypted_data_key, created_at, last_activity,
                   cwd, env, claude_version, shell
            FROM sessions WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions SET status = ?1, last_activity = datetime('now')
            WHERE id = ?2
            "#,
        )
        .bind(status.to_string())
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_session_cwd(&self, id: &str, cwd: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions SET cwd = ?1, last_activity = datetime('now')
            WHERE id = ?2
            "#,
        )
        .bind(cwd)
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_session_machine(
        &self,
        id: &str,
        machine_id: &str,
        machine_name: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions SET machine_id = ?1, machine_name = ?2, last_activity = datetime('now')
            WHERE id = ?3
            "#,
        )
        .bind(machine_id)
        .bind(machine_name)
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_session(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM sessions WHERE id = ?1
            "#,
        )
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_sessions_by_user(&self, user_id: &str) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT id, tag, user_id, machine_id, machine_name, status,
                   encrypted_data_key, created_at, last_activity,
                   cwd, env, claude_version, shell
            FROM sessions WHERE user_id = ?1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn list_active_sessions_by_machine(&self, machine_id: &str) -> Result<Vec<Session>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT id, tag, user_id, machine_id, machine_name, status,
                   encrypted_data_key, created_at, last_activity,
                   cwd, env, claude_version, shell
            FROM sessions
            WHERE machine_id = ?1 AND status IN ('initializing', 'running', 'paused')
            ORDER BY created_at DESC
            "#,
        )
        .bind(machine_id)
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // Machine operations
    pub async fn create_machine(&self, machine: &Machine) -> Result<()> {
        let capabilities_str = machine
            .capabilities
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");

        sqlx::query(
            r#"
            INSERT INTO machines (id, user_id, name, public_key, platform, capabilities, hostname)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&machine.id)
        .bind(&machine.user_id)
        .bind(&machine.name)
        .bind(&machine.public_key)
        .bind(machine.platform.to_string())
        .bind(capabilities_str)
        .bind(&machine.hostname)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_machine(&self, id: &str) -> Result<Option<Machine>> {
        let row: Option<MachineRow> = sqlx::query_as(
            r#"
            SELECT id, user_id, name, public_key, platform,
                   capabilities, last_seen, hostname
            FROM machines WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_machines_by_user(&self, user_id: &str) -> Result<Vec<Machine>> {
        let rows: Vec<MachineRow> = sqlx::query_as(
            r#"
            SELECT id, user_id, name, public_key, platform,
                   capabilities, last_seen, hostname
            FROM machines WHERE user_id = ?1
            ORDER BY last_seen DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn touch_machine(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE machines SET last_seen = datetime('now') WHERE id = ?1
            "#,
        )
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_machine_name(&self, id: &str, name: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE machines SET name = ?1, last_seen = datetime('now') WHERE id = ?2
            "#,
        )
        .bind(name)
        .bind(id)
        .execute(&*self.pool)
        .await?;

        // Also update machine_name in all sessions for this machine
        sqlx::query(
            r#"
            UPDATE sessions SET machine_name = ?1 WHERE machine_id = ?2
            "#,
        )
        .bind(name)
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_machine(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM machines WHERE id = ?1
            "#,
        )
        .bind(id)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
}

// Helper structs for sqlx query_as
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    tag: String,
    user_id: String,
    machine_id: String,
    machine_name: String,
    status: String,
    encrypted_data_key: Option<Vec<u8>>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_activity: chrono::DateTime<chrono::Utc>,
    cwd: String,
    env: String,
    claude_version: Option<String>,
    shell: String,
}

impl From<SessionRow> for Session {
    fn from(r: SessionRow) -> Self {
        Session {
            id: r.id,
            tag: r.tag,
            user_id: r.user_id,
            machine_id: r.machine_id,
            machine_name: r.machine_name,
            status: parse_session_status(&r.status),
            encrypted_data_key: r.encrypted_data_key,
            created_at: r.created_at,
            last_activity: r.last_activity,
            metadata: happy_core::SessionMetadata {
                cwd: r.cwd,
                env: serde_json::from_str(&r.env).unwrap_or_default(),
                claude_version: r.claude_version,
                shell: r.shell,
            },
        }
    }
}

#[derive(sqlx::FromRow)]
struct MachineRow {
    id: String,
    user_id: String,
    name: String,
    public_key: Vec<u8>,
    platform: String,
    capabilities: String,
    last_seen: chrono::DateTime<chrono::Utc>,
    hostname: Option<String>,
}

impl From<MachineRow> for Machine {
    fn from(r: MachineRow) -> Self {
        let capabilities = r
            .capabilities
            .split(',')
            .filter_map(|s| match s {
                "terminal" => Some(happy_core::Capability::Terminal),
                "file_system" => Some(happy_core::Capability::FileSystem),
                "notifications" => Some(happy_core::Capability::Notifications),
                "voice" => Some(happy_core::Capability::Voice),
                _ => None,
            })
            .collect();

        Machine {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            public_key: r.public_key,
            platform: parse_platform(&r.platform),
            last_seen: r.last_seen,
            capabilities,
            ip_address: None,
            hostname: r.hostname,
        }
    }
}

fn parse_session_status(s: &str) -> SessionStatus {
    match s {
        "initializing" => SessionStatus::Initializing,
        "running" => SessionStatus::Running,
        "paused" => SessionStatus::Paused,
        "terminated" => SessionStatus::Terminated,
        _ => SessionStatus::Initializing,
    }
}

fn parse_platform(s: &str) -> Platform {
    match s {
        "macos" => Platform::MacOS,
        "linux" => Platform::Linux,
        "windows" => Platform::Windows,
        _ => Platform::Linux,
    }
}
