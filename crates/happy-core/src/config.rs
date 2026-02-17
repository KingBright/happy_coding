//! Configuration management for Happy Coding

use crate::error::{HappyError, Result};
use crate::types::ProjectConfig;
use std::path::{Path, PathBuf};

/// Configuration file names to search for
pub const CONFIG_FILE_NAMES: &[&str] =
    &["happy.config.yaml", "happy.config.yml", "happy.config.json"];

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
        let mut config: ProjectConfig = if config_path
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        // Resolve skill paths
        let base_dir = config_path.parent().unwrap_or(Path::new("."));
        for skill in &mut config.skills {
            if skill.prompt.is_none() {
                if let Some(ref path_str) = skill.path {
                    let skill_path = base_dir.join(path_str);
                    // Check if it's a directory (look for SKILL.md) or a file
                    let final_path = if skill_path.is_dir() {
                        skill_path.join("SKILL.md")
                    } else {
                        skill_path.clone()
                    };

                    if final_path.exists() {
                        let raw_content =
                            std::fs::read_to_string(&final_path).map_err(|e| HappyError::Io(e))?;

                        // Strip frontmatter if present to avoid duplication on rebuild
                        let content = if raw_content.starts_with("---") {
                            // Find the second "---"
                            if let Some(end_idx) = raw_content[3..].find("\n---") {
                                let content_start = 3 + end_idx + 4; // skip "\n---"
                                raw_content[content_start..].trim_start().to_string()
                            } else {
                                raw_content
                            }
                        } else {
                            raw_content
                        };

                        skill.prompt = Some(content);
                    }
                }
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_skill_strips_frontmatter() -> Result<()> {
        // Create a unique temp dir manually since tempfile header is not available
        let temp_dir_name = format!(
            "test_skill_load_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(temp_dir_name);
        std::fs::create_dir_all(&temp_dir)?;

        let skill_dir = temp_dir.join("skills/test-skill");
        std::fs::create_dir_all(&skill_dir)?;

        let skill_md_path = skill_dir.join("SKILL.md");
        // Simulate a file that has been processed by AntigravityAdapter before
        let content = r#"---
name: test-skill
description: A test skill
---

# Actual Content
This is the content that should be preserved.
"#;
        std::fs::write(&skill_md_path, content)?;

        let config_content = r#"
name: test-project
version: 1.0.0
skills:
  - name: test-skill
    description: A test skill
    path: skills/test-skill
"#;
        let config_path = temp_dir.join("happy.config.yaml");
        std::fs::write(&config_path, config_content)?;

        let mut manager = ConfigManager::new();
        let config_result = manager.load(&config_path);

        // Clean up before unwrapping to ensure cleanup happens if load succeeds
        // If load fails, we still want to see the error, so we unwrap after

        if config_result.is_err() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let config = config_result?;

        assert_eq!(config.skills.len(), 1);
        let skill = &config.skills[0];
        assert!(skill.prompt.is_some());

        let prompt = skill.prompt.as_ref().unwrap();
        // Check that duplicated frontmatter is removed
        // The loader should strip the first YAML block
        assert!(
            !prompt.contains("name: test-skill"),
            "Prompt should not contain frontmatter: {}",
            prompt
        );
        assert!(prompt.contains("# Actual Content"));
        assert_eq!(
            prompt.trim(),
            "# Actual Content\nThis is the content that should be preserved."
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }
}
