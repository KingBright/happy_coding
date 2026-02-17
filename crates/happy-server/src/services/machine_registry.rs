//! Machine registry service

use crate::storage::{Database, MemoryCache};
use anyhow::Result;
use happy_core::{Machine, MachineInfo, Platform};
use std::sync::Arc;
use tracing::{debug, info};

pub struct MachineRegistry {
    db: Arc<Database>,
    cache: Arc<MemoryCache>,
}

impl MachineRegistry {
    pub fn new(db: Arc<Database>, cache: Arc<MemoryCache>) -> Self {
        Self { db, cache }
    }

    pub async fn register_machine(
        &self,
        user_id: &str,
        machine_id: &str,
        name: &str,
        platform: Platform,
    ) -> Result<Machine> {
        info!("Registering machine: user={}, id={}, name={}", user_id, machine_id, name);

        // Check if machine already exists
        if let Some(mut existing) = self.db.get_machine(machine_id).await? {
            info!("Machine {} already registered, updating name and last_seen", machine_id);

            // Update name if changed
            if existing.name != name {
                existing.name = name.to_string();
                self.db.update_machine_name(machine_id, name).await?;
            }

            self.update_machine_status(machine_id, true).await?;

            // Update cache
            let machine_key = format!("machine:{}", machine_id);
            let machine_json = serde_json::to_vec(&existing)?;
            self.cache.set(machine_key, machine_json);

            return Ok(existing);
        }

        let machine = Machine::new(
            machine_id.to_string(),
            user_id.to_string(),
            name.to_string(),
            vec![], // No public key for CLI machines
            platform,
        );

        // Save to database
        self.db.create_machine(&machine).await?;

        // Cache machine
        let machine_key = format!("machine:{}", machine.id);
        let machine_json = serde_json::to_vec(&machine)?;
        self.cache.set(machine_key, machine_json);

        Ok(machine)
    }

    pub async fn get_machine(&self, id: &str) -> Result<Option<Machine>> {
        // Try cache first
        let machine_key = format!("machine:{}", id);
        if let Some(data) = self.cache.get(&machine_key) {
            if let Ok(machine) = serde_json::from_slice::<Machine>(&data) {
                return Ok(Some(machine));
            }
        }

        // Fall back to database
        self.db.get_machine(id).await
    }

    pub async fn update_machine_status(&self, id: &str, is_online: bool) -> Result<()> {
        debug!("Updating machine {} status: online={}", id, is_online);

        if is_online {
            self.db.touch_machine(id).await?;
            // Update cache timestamp
            let machine_key = format!("machine:{}", id);
            if let Some(data) = self.cache.get(&machine_key) {
                if let Ok(mut machine) = serde_json::from_slice::<Machine>(&data) {
                    machine.last_seen = chrono::Utc::now();
                    let machine_json = serde_json::to_vec(&machine)?;
                    self.cache.set(machine_key, machine_json);
                }
            }
        }

        // Update online status in cache
        let status_key = format!("machine:{}:online", id);
        if is_online {
            self.cache.set(status_key, vec![1]);
        } else {
            self.cache.delete(&status_key);
        }

        Ok(())
    }

    pub async fn list_user_machines(&self, user_id: &str) -> Result<Vec<MachineInfo>> {
        let machines = self.db.list_machines_by_user(user_id).await?;

        // Convert to MachineInfo
        let infos: Vec<MachineInfo> = machines
            .into_iter()
            .map(|m| {
                let status_key = format!("machine:{}:online", m.id);
                let is_online = self.cache.exists(&status_key);
                MachineInfo {
                    id: m.id,
                    name: m.name,
                    platform: m.platform,
                    last_seen: m.last_seen,
                    is_online,
                    capabilities: m.capabilities,
                }
            })
            .collect();

        Ok(infos)
    }

    pub async fn unregister_machine(&self, id: &str) -> Result<()> {
        info!("Unregistering machine: {}", id);

        self.db.delete_machine(id).await?;

        // Remove from cache
        let machine_key = format!("machine:{}", id);
        let status_key = format!("machine:{}:online", id);
        self.cache.delete(&machine_key);
        self.cache.delete(&status_key);

        Ok(())
    }
}
