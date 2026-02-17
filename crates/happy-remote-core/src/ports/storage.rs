//! Storage traits for persistence

use happy_types::{AccessKey, Machine, Session, User};
use crate::Result;
use async_trait::async_trait;

/// User store
#[async_trait]
pub trait UserStore: Send + Sync {
    async fn create_user(&self, user: &User) -> Result<()>;
    async fn get_user(&self, id: &str) -> Result<Option<User>>;
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn update_user(&self, user: &User) -> Result<()>;
    async fn delete_user(&self, id: &str) -> Result<()>;
}

/// Session store
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create_session(&self, session: &Session) -> Result<()>;
    async fn get_session(&self, id: &str) -> Result<Option<Session>>;
    async fn update_session(&self, session: &Session) -> Result<()>;
    async fn delete_session(&self, id: &str) -> Result<()>;
    async fn list_sessions_by_user(&self, user_id: &str) -> Result<Vec<Session>>;
    async fn list_active_sessions_by_machine(&self, machine_id: &str) -> Result<Vec<Session>>;
}

/// Machine store
#[async_trait]
pub trait MachineStore: Send + Sync {
    async fn create_machine(&self, machine: &Machine) -> Result<()>;
    async fn get_machine(&self, id: &str) -> Result<Option<Machine>>;
    async fn update_machine(&self, machine: &Machine) -> Result<()>;
    async fn delete_machine(&self, id: &str) -> Result<()>;
    async fn list_machines_by_user(&self, user_id: &str) -> Result<Vec<Machine>>;
    async fn touch_machine(&self, id: &str) -> Result<()>;
}

/// Access key store
#[async_trait]
pub trait AccessKeyStore: Send + Sync {
    async fn create_access_key(&self, key: &AccessKey) -> Result<()>;
    async fn get_access_key(&self, id: &str) -> Result<Option<AccessKey>>;
    async fn get_access_key_by_key(&self, key: &str) -> Result<Option<AccessKey>>;
    async fn update_access_key(&self, key: &AccessKey) -> Result<()>;
    async fn delete_access_key(&self, id: &str) -> Result<()>;
    async fn list_access_keys_by_user(&self, user_id: &str) -> Result<Vec<AccessKey>>;
    async fn touch_access_key(&self, id: &str) -> Result<()>;
}
