//! Configuration management

use anyhow::{Context, Result};
use happy_core::Settings;
use std::path::PathBuf;

pub struct SettingsManager;

impl SettingsManager {
    /// Get the happy home directory (~/.happy)
    pub fn happy_home() -> Result<PathBuf> {
        if let Ok(path) = std::env::var("HAPPY_HOME") {
            return Ok(PathBuf::from(path));
        }
        let home = dirs::home_dir().context("Could not find home directory")?;
        Ok(home.join(".happy"))
    }

    /// Get the settings file path
    pub fn settings_path() -> Result<PathBuf> {
        Ok(Self::happy_home()?.join("settings.json"))
    }

    /// Get the log file path
    #[allow(dead_code)]
    pub fn log_path() -> Result<PathBuf> {
        Ok(Self::happy_home()?.join("daemon.log"))
    }

    /// Get the daemon PID file path
    #[allow(dead_code)]
    pub fn pid_path() -> Result<PathBuf> {
        Ok(Self::happy_home()?.join("daemon.pid"))
    }

    /// Get the private key file path
    #[allow(dead_code)]
    pub fn private_key_path() -> Result<PathBuf> {
        Ok(Self::happy_home()?.join("access.key"))
    }

    /// Get the machine ID file path
    pub fn machine_id_path() -> Result<PathBuf> {
        Ok(Self::happy_home()?.join("machine_id"))
    }

    /// Load settings from disk
    pub fn load() -> Result<Settings> {
        let path = Self::settings_path()?;

        let content = if !path.exists() {
            // Create default settings
            let settings = Settings::default();
            serde_json::to_string_pretty(&settings)?
        } else {
            std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read settings from {:?}", path))?
        };

        // Check if we need migration (raw JSON contains machine_id)
        let raw_val: serde_json::Value = serde_json::from_str(&content)?;
        let is_old_format = raw_val.get("machine_id").is_some();

        let mut settings: Settings = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse settings from {:?}", path))?;

        // The machine_id is already skipped by #[serde(skip)]

        // Load or generate machine_id
        let id_path = Self::machine_id_path()?;
        if id_path.exists() {
            settings.machine_id = std::fs::read_to_string(&id_path)
                .with_context(|| format!("Failed to read machine_id from {:?}", id_path))?
                .trim()
                .to_string();
        } else {
            settings.machine_id = uuid::Uuid::new_v4().to_string();
            std::fs::write(&id_path, &settings.machine_id)
                .with_context(|| format!("Failed to write machine_id to {:?}", id_path))?;
        }

        if !path.exists() || is_old_format {
            Self::save(&settings)?;
        }

        Ok(settings)
    }

    /// Save settings to disk
    pub fn save(settings: &Settings) -> Result<()> {
        let path = Self::settings_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        let content =
            serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write settings to {:?}", path))?;

        // Set permissions on Unix (restrict to owner only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }
}

// Re-export from happy_core for convenience
pub use happy_core::AIProvider;
