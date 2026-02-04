//! Configuration management for Happy Coding

use crate::error::{HappyError, Result};
use crate::types::ProjectConfig;
use std::path::{Path, PathBuf};

/// Configuration file names to search for
pub const CONFIG_FILE_NAMES: &[&str] = &[
    "happy.config.yaml",
    "happy.config.yml",
    "happy.config.json",
];

/// Configuration manager for loading and saving project configurations
pub struct ConfigManager {
    cache: std::collections::HashMap<PathBuf, CachedConfig>,
}

struct CachedConfig {
    config: ProjectConfig,
    modified_time: std::time::SystemTime,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    /// Find configuration file in a directory
    pub fn find_config_file(dir: &Path) -> Option<PathBuf> {
        for name in CONFIG_FILE_NAMES {
            let path = dir.join(name);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Load configuration from a file
    pub fn load(&mut self, config_path: &Path) -> Result<ProjectConfig> {
        let metadata = std::fs::metadata(config_path)?;
        let modified_time = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        // Check cache
        if let Some(cached) = self.cache.get(config_path) {
            if cached.modified_time == modified_time {
                return Ok(cached.config.clone());
            }
        }

        // Load and parse
        let content = std::fs::read_to_string(config_path)?;
        let config: ProjectConfig = if config_path
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        // Cache the result
        self.cache.insert(
            config_path.to_path_buf(),
            CachedConfig {
                config: config.clone(),
                modified_time,
            },
        );

        Ok(config)
    }

    /// Load configuration from a directory (searches for config files)
    pub fn load_from_directory(&mut self, dir: &Path) -> Result<(ProjectConfig, PathBuf)> {
        let config_path = Self::find_config_file(dir)
            .ok_or_else(|| HappyError::ConfigNotFound(dir.display().to_string()))?;

        let config = self.load(&config_path)?;
        Ok((config, config_path))
    }

    /// Validate a configuration
    pub fn validate(&self, config: &ProjectConfig) -> crate::types::ValidationResult {
        use crate::types::{ValidationError, ValidationResult, ValidationWarning};

        let mut result = ValidationResult::ok();

        // Check name format
        let name_regex = regex::Regex::new(r"^[a-z0-9][a-z0-9-]*$").unwrap();
        if !name_regex.is_match(&config.name) {
            result = result.with_error(ValidationError {
                field: "name".to_string(),
                message: "Project name must be lowercase alphanumeric with hyphens".to_string(),
                code: "INVALID_NAME".to_string(),
            });
        }

        // Check version format
        let version_regex = regex::Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
        if !version_regex.is_match(&config.version) {
            result = result.with_error(ValidationError {
                field: "version".to_string(),
                message: "Version must follow semver format (e.g., 1.0.0)".to_string(),
                code: "INVALID_VERSION".to_string(),
            });
        }

        // Check enabled platforms
        if config.targets.enabled_platforms().is_empty() {
            result = result.with_warning(ValidationWarning {
                field: "targets".to_string(),
                message: "No target platforms are enabled".to_string(),
                suggestion: Some("Enable at least one target platform".to_string()),
            });
        }

        // Check skill names
        for skill in &config.skills {
            if !name_regex.is_match(&skill.name) {
                result = result.with_error(ValidationError {
                    field: format!("skills.{}", skill.name),
                    message: "Skill name must be lowercase alphanumeric with hyphens".to_string(),
                    code: "INVALID_SKILL_NAME".to_string(),
                });
            }
        }

        // Check workflow skill references
        let skill_names: std::collections::HashSet<_> =
            config.skills.iter().map(|s| &s.name).collect();
        for workflow in &config.workflows {
            for step in &workflow.steps {
                if let Some(ref skill_name) = step.skill {
                    if !skill_names.contains(skill_name) {
                        result = result.with_warning(ValidationWarning {
                            field: format!("workflows.{}.steps", workflow.name),
                            message: format!("Workflow references unknown skill: {}", skill_name),
                            suggestion: Some(format!(
                                "Define skill '{}' or use an existing skill",
                                skill_name
                            )),
                        });
                    }
                }
            }
        }

        // Check command references
        let workflow_names: std::collections::HashSet<_> =
            config.workflows.iter().map(|w| &w.name).collect();
        for command in &config.commands {
            if let Some(ref workflow_name) = command.workflow {
                if !workflow_names.contains(workflow_name) {
                    result = result.with_warning(ValidationWarning {
                        field: format!("commands.{}.workflow", command.name),
                        message: format!("Command references unknown workflow: {}", workflow_name),
                        suggestion: Some(format!(
                            "Define workflow '{}' or use an existing workflow",
                            workflow_name
                        )),
                    });
                }
            }
            if let Some(ref skill_name) = command.skill {
                if !skill_names.contains(skill_name) {
                    result = result.with_warning(ValidationWarning {
                        field: format!("commands.{}.skill", command.name),
                        message: format!("Command references unknown skill: {}", skill_name),
                        suggestion: Some(format!(
                            "Define skill '{}' or use an existing skill",
                            skill_name
                        )),
                    });
                }
            }
        }

        result
    }

    /// Save configuration to a file
    pub fn save(&self, config: &ProjectConfig, config_path: &Path) -> Result<()> {
        let content = if config_path
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            serde_json::to_string_pretty(config)?
        } else {
            serde_yaml::to_string(config)?
        };

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(config_path, content)?;

        Ok(())
    }

    /// Create a default configuration
    pub fn create_default(name: &str) -> ProjectConfig {
        ProjectConfig::new(name)
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}
