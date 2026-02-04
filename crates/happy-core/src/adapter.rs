//! Platform adapter trait and factory

use std::path::Path;
use async_trait::async_trait;
use crate::error::Result;
use crate::types::{
    Platform, Feature, ProjectConfig, BuildResult, ValidationResult, InstallTarget,
};

/// Platform adapter trait
///
/// Each platform (Claude, Codex, Antigravity, IDX) implements this trait
/// to provide platform-specific build and install functionality.
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Get the platform this adapter handles
    fn platform(&self) -> Platform;

    /// Get the features supported by this platform
    fn supported_features(&self) -> &[Feature];

    /// Get the limitations of this platform
    fn limitations(&self) -> &[&str];

    /// Build the configuration for this platform
    async fn build(&self, config: &ProjectConfig, output_dir: &Path) -> Result<BuildResult>;

    /// Install built artifacts to target location
    async fn install(&self, source: &Path, target: &InstallTarget) -> Result<()>;

    /// Validate configuration for this platform
    fn validate(&self, config: &ProjectConfig) -> ValidationResult;

    /// Detect if this platform's tools are installed
    async fn detect(&self) -> bool {
        false
    }

    /// Get the global install path for this platform
    fn global_install_path(&self) -> Option<std::path::PathBuf>;
}

/// Adapter factory for creating and managing platform adapters
pub struct AdapterFactory {
    adapters: std::collections::HashMap<Platform, Box<dyn Adapter>>,
}

impl Default for AdapterFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterFactory {
    /// Create a new adapter factory
    pub fn new() -> Self {
        Self {
            adapters: std::collections::HashMap::new(),
        }
    }

    /// Register an adapter
    pub fn register(&mut self, adapter: Box<dyn Adapter>) {
        self.adapters.insert(adapter.platform(), adapter);
    }

    /// Get an adapter for a platform
    pub fn get(&self, platform: Platform) -> Option<&dyn Adapter> {
        self.adapters.get(&platform).map(|a| a.as_ref())
    }

    /// Check if an adapter is registered for a platform
    pub fn has(&self, platform: Platform) -> bool {
        self.adapters.contains_key(&platform)
    }

    /// Get all registered platforms
    pub fn registered_platforms(&self) -> Vec<Platform> {
        self.adapters.keys().copied().collect()
    }

    /// Get all adapters
    pub fn all(&self) -> impl Iterator<Item = &dyn Adapter> {
        self.adapters.values().map(|a| a.as_ref())
    }

    /// Get supported features for a platform
    pub fn supported_features(&self, platform: Platform) -> Option<&[Feature]> {
        self.get(platform).map(|a| a.supported_features())
    }

    /// Detect available platforms
    pub async fn detect_available(&self) -> Vec<Platform> {
        let mut available = Vec::new();
        for (platform, adapter) in &self.adapters {
            if adapter.detect().await {
                available.push(*platform);
            }
        }
        available
    }
}
