//! API client for Happy Remote server

use anyhow::{Context, Result};
use happy_core::{AuthTokens, User};
use reqwest::Client as ReqwestClient;

#[allow(dead_code)]
pub struct Client {
    http: ReqwestClient,
    base_url: String,
}

impl Client {
    pub fn new() -> Self {
        // Try to load server URL from settings, fallback to default
        let base_url = crate::config::SettingsManager::load()
            .ok()
            .map(|s| s.server_url)
            .unwrap_or_else(|| "https://api.happy-remote.dev".to_string());

        Self {
            http: ReqwestClient::new(),
            base_url,
        }
    }

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// Create client from settings explicitly
    pub fn from_settings() -> Self {
        Self::new()
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthTokens> {
        // server_url already includes /api/v1, so just append the endpoint
        let url = format!("{}/auth/login", self.base_url);

        let response = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "email": email,
                "password": password,
            }))
            .send()
            .await
            .context("Failed to send login request")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let error: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            anyhow::bail!(
                "Login failed: {}",
                error["message"].as_str().unwrap_or("Invalid credentials")
            );
        }

        // Server returns LoginResponse { access_token, refresh_token, expires_in, user }
        let login_response: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse login response")?;

        let tokens = AuthTokens {
            access_token: login_response["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing access_token in response"))?
                .to_string(),
            refresh_token: login_response["refresh_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing refresh_token in response"))?
                .to_string(),
            expires_in: login_response["expires_in"].as_i64().unwrap_or(0),
        };

        Ok(tokens)
    }

    pub async fn register(
        &self,
        email: &str,
        password: &str,
        name: Option<&str>,
    ) -> Result<AuthTokens> {
        let response = self
            .http
            .post(format!("{}/auth/register", self.base_url))
            .json(&serde_json::json!({
                "email": email,
                "password": password,
                "name": name,
            }))
            .send()
            .await
            .context("Failed to send register request")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let error: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            anyhow::bail!(
                "Registration failed: {}",
                error["message"].as_str().unwrap_or("Unknown error")
            );
        }

        // Server returns LoginResponse { access_token, refresh_token, expires_in, user }
        let login_response: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse register response")?;

        let tokens = AuthTokens {
            access_token: login_response["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing access_token in response"))?
                .to_string(),
            refresh_token: login_response["refresh_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing refresh_token in response"))?
                .to_string(),
            expires_in: login_response["expires_in"].as_i64().unwrap_or(0),
        };

        Ok(tokens)
    }

    pub async fn logout(&self, token: &str) -> Result<()> {
        let _ = self
            .http
            .post(format!("{}/auth/logout", self.base_url))
            .bearer_auth(token)
            .send()
            .await;
        Ok(())
    }

    pub async fn get_user_info(&self, token: &str) -> Result<User> {
        let response = self
            .http
            .get(format!("{}/users/me", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to get user info")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get user info: {}", response.status());
        }

        let user: User = response.json().await?;
        Ok(user)
    }

    pub async fn list_access_keys(&self, token: &str) -> Result<Vec<AccessKeyInfo>> {
        let response = self
            .http
            .get(format!("{}/access-keys", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to list access keys")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to list access keys: {}", response.status());
        }

        let keys: Vec<AccessKeyInfo> = response.json().await?;
        Ok(keys)
    }

    pub async fn send_notification(&self, token: &str, message: &str) -> Result<()> {
        let _ = self
            .http
            .post(format!("{}/push/send", self.base_url))
            .bearer_auth(token)
            .json(&serde_json::json!({
                "message": message,
            }))
            .send()
            .await;
        Ok(())
    }

    /// Create a new session on the server
    pub async fn create_session(
        &self,
        token: &str,
        tag: &str,
        profile: Option<&str>,
        machine_id: &str,
        machine_name: &str,
        cwd: &str,
    ) -> Result<SessionInfo> {
        let response = self
            .http
            .post(format!("{}/sessions", self.base_url))
            .bearer_auth(token)
            .header("X-Machine-ID", machine_id)
            .header("X-Machine-Name", machine_name)
            .json(&serde_json::json!({
                "tag": tag,
                "profile": profile,
                "cwd": cwd,
            }))
            .send()
            .await
            .context("Failed to create session")?;

        if !response.status().is_success() {
            let error: serde_json::Value = response.json().await?;
            anyhow::bail!(
                "Failed to create session: {}",
                error["message"].as_str().unwrap_or("Unknown error")
            );
        }

        let result: SessionResponse = response.json().await?;
        Ok(SessionInfo {
            id: result.session.id,
            tag: result.session.tag,
        })
    }

    /// List all sessions for the user
    pub async fn list_sessions(&self, token: &str) -> Result<Vec<SessionInfo>> {
        let response = self
            .http
            .get(format!("{}/sessions", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to list sessions")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to list sessions: {}", response.status());
        }

        let result: SessionsListResponse = response.json().await?;
        Ok(result
            .sessions
            .into_iter()
            .map(|s| SessionInfo {
                id: s.id,
                tag: s.tag,
            })
            .collect())
    }

    /// Delete a session
    pub async fn delete_session(&self, token: &str, session_id: &str) -> Result<()> {
        let response = self
            .http
            .delete(format!("{}/sessions/{}", self.base_url, session_id))
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to delete session")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to delete session: {}", response.status());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub tag: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SessionResponse {
    pub session: SessionDetails,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SessionsListResponse {
    pub sessions: Vec<SessionDetails>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SessionDetails {
    pub id: String,
    pub tag: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccessKeyInfo {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_revoked: bool,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
