//! Build coordinator for Happy Coding

use std::path::Path;
use std::time::Instant;
use crate::adapter::AdapterFactory;
use crate::error::{HappyError, Result};
use crate::types::{BuildOptions, BuildResult, BuildSummary, Platform, ProjectConfig};

/// Build coordinator that orchestrates builds across multiple platforms
pub struct Builder {
    adapter_factory: AdapterFactory,
}

impl Builder {
    /// Create a new builder with the given adapter factory
    pub fn new(adapter_factory: AdapterFactory) -> Self {
        Self { adapter_factory }
    }

    /// Build for all enabled platforms
    pub async fn build(
        &self,
        config: &ProjectConfig,
        project_dir: &Path,
        options: &BuildOptions,
    ) -> Result<BuildSummary> {
        let start = Instant::now();

        // Determine platforms to build
        let platforms = if let Some(target) = options.target {
            vec![target]
        } else {
            config.targets.enabled_platforms()
        };

        if platforms.is_empty() {
            return Err(HappyError::Build {
                platform: "all".to_string(),
                message: "No platforms enabled for build".to_string(),
            });
        }

        // Validate adapters exist
        for platform in &platforms {
            if !self.adapter_factory.has(*platform) {
                return Err(HappyError::AdapterNotFound(platform.to_string()));
            }
        }

        // Run builds
        let mut results = Vec::new();
        for platform in &platforms {
            let output_dir = project_dir.join(config.output_dir(*platform));
            
            // Clean if requested
            if options.clean && output_dir.exists() {
                std::fs::remove_dir_all(&output_dir)?;
            }

            let result = match self.adapter_factory.get(*platform) {
                Some(adapter) => adapter.build(config, &output_dir).await,
                None => Err(HappyError::AdapterNotFound(platform.to_string())),
            };

            results.push(result.unwrap_or_else(|e| {
                BuildResult::failure(*platform, vec![e.to_string()])
            }));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = results.iter().all(|r| r.success);

        Ok(BuildSummary {
            success,
            results,
            duration_ms,
            platforms,
        })
    }

    /// Build for a single platform
    pub async fn build_platform(
        &self,
        config: &ProjectConfig,
        project_dir: &Path,
        platform: Platform,
    ) -> Result<BuildResult> {
        let adapter = self
            .adapter_factory
            .get(platform)
            .ok_or_else(|| HappyError::AdapterNotFound(platform.to_string()))?;

        let output_dir = project_dir.join(config.output_dir(platform));
        adapter.build(config, &output_dir).await
    }

    /// Validate configuration for all enabled platforms
    pub fn validate(&self, config: &ProjectConfig) -> Vec<(Platform, crate::types::ValidationResult)> {
        let mut results = Vec::new();

        for platform in config.targets.enabled_platforms() {
            if let Some(adapter) = self.adapter_factory.get(platform) {
                results.push((platform, adapter.validate(config)));
            }
        }

        results
    }

    /// Format build summary for display
    pub fn format_summary(&self, summary: &BuildSummary) -> String {
        let mut lines = Vec::new();
        
        lines.push(String::new());
        lines.push("╔════════════════════════════════════════════════════════╗".to_string());
        lines.push("║  Build Summary                                         ║".to_string());
        lines.push("╠════════════════════════════════════════════════════════╣".to_string());
        
        let status = if summary.success { "✅ SUCCESS" } else { "❌ FAILED" };
        lines.push(format!("║  Status:   {:<45}║", status));
        lines.push(format!("║  Duration: {:<45}║", format!("{}ms", summary.duration_ms)));
        
        let platforms_str: String = summary.platforms.iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("║  Platforms: {:<44}║", platforms_str));
        
        lines.push("╠════════════════════════════════════════════════════════╣".to_string());

        for result in &summary.results {
            let icon = if result.success { "✅" } else { "❌" };
            let file_count = result.files.len();
            let warning_count = result.warnings.len();
            let error_count = result.errors.len();
            
            lines.push(format!(
                "║  {} {:<10} │ Files: {:<3} │ ⚠️ {:<3} │ ❌ {:<3}      ║",
                icon,
                result.platform.as_str(),
                file_count,
                warning_count,
                error_count
            ));
        }

        lines.push("╚════════════════════════════════════════════════════════╝".to_string());
        lines.push(String::new());

        lines.join("\n")
    }
}
