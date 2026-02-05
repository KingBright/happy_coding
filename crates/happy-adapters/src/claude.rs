//! Claude Code adapter
//!
//! Generates configuration for Claude Code CLI and Desktop:
//! - `.claude/skills/<name>/SKILL.md` - Skill definitions
//! - `.claude/settings.json` - Settings with hooks
//! - `.claude/mcp.json` - MCP server configurations

use async_trait::async_trait;
use happy_core::{
    Adapter, BuildResult, Feature, HappyError, InstallTarget, Platform, ProjectConfig, Result,
    SkillDefinition, ValidationResult,
};
use std::path::{Path, PathBuf};

/// Claude Code platform adapter
pub struct ClaudeAdapter;

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Generate SKILL.md content for a skill
    fn generate_skill_md(&self, skill: &SkillDefinition) -> String {
        let mut content = String::new();

        // YAML frontmatter
        content.push_str("---\n");
        content.push_str(&format!("name: {}\n", skill.name));
        content.push_str(&format!("description: {}\n", skill.description));
        if !skill.tags.is_empty() {
            content.push_str(&format!("tags: [{}]\n", skill.tags.join(", ")));
        }
        content.push_str("---\n\n");

        // Prompt content
        if let Some(ref prompt) = skill.prompt {
            content.push_str(prompt);
        }
        content.push('\n');

        // Parameters section
        if !skill.parameters.is_empty() {
            content.push_str("\n## Parameters\n\n");
            for param in &skill.parameters {
                let required = if param.required { " (required)" } else { "" };
                content.push_str(&format!(
                    "- **{}**{}: {}\n",
                    param.name, required, param.description
                ));
            }
        }

        // Examples section
        if !skill.examples.is_empty() {
            content.push_str("\n## Examples\n\n");
            for (i, example) in skill.examples.iter().enumerate() {
                if let Some(ref desc) = example.description {
                    content.push_str(&format!("### Example {}: {}\n\n", i + 1, desc));
                } else {
                    content.push_str(&format!("### Example {}\n\n", i + 1));
                }
                content.push_str("**Input:**\n");
                content.push_str(&format!("```\n{}\n```\n\n", example.input));
                content.push_str("**Output:**\n");
                content.push_str(&format!("```\n{}\n```\n\n", example.output));
            }
        }

        content
    }

    /// Generate settings.json content
    fn generate_settings(&self, config: &ProjectConfig) -> String {
        let mut settings = serde_json::Map::new();

        // Add permissions if defined
        settings.insert(
            "permissions".to_string(),
            serde_json::json!({
                "allowedTools": ["Read", "Write", "Bash(*)"]
            }),
        );

        // Add MCP configuration if present
        if let Some(ref mcp) = config.mcp {
            let mut servers = serde_json::Map::new();
            for server in &mcp.servers {
                let mut server_config = serde_json::Map::new();
                if let Some(ref cmd) = server.command {
                    server_config.insert(
                        "command".to_string(),
                        serde_json::Value::String(cmd.clone()),
                    );
                }
                if !server.args.is_empty() {
                    server_config.insert("args".to_string(), serde_json::json!(server.args));
                }
                if let Some(ref url) = server.url {
                    server_config.insert("url".to_string(), serde_json::Value::String(url.clone()));
                }
                if !server.env.is_empty() {
                    server_config.insert("env".to_string(), serde_json::json!(server.env));
                }
                servers.insert(
                    server.name.clone(),
                    serde_json::Value::Object(server_config),
                );
            }
            settings.insert("mcpServers".to_string(), serde_json::Value::Object(servers));
        }

        serde_json::to_string_pretty(&settings).unwrap_or_default()
    }

    /// Ensure directory exists
    async fn ensure_dir(&self, path: &Path) -> Result<()> {
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    /// Write file with content
    async fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            self.ensure_dir(parent).await?;
        }
        tokio::fs::write(path, content).await?;
        Ok(())
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Adapter for ClaudeAdapter {
    fn platform(&self) -> Platform {
        Platform::Claude
    }

    fn supported_features(&self) -> &[Feature] {
        &[
            Feature::Skill,
            Feature::Workflow,
            Feature::Command,
            Feature::Hooks,
            Feature::Mcp,
        ]
    }

    fn limitations(&self) -> &[&str] {
        &[
            "Skills with 'allowed-tools' frontmatter may not work in Agent SDK",
            "Nested slash commands have known issues",
        ]
    }

    async fn build(&self, config: &ProjectConfig, output_dir: &Path) -> Result<BuildResult> {
        let mut files = Vec::new();

        // Ensure output directory exists
        self.ensure_dir(output_dir).await?;

        // Always create skills directory, even if empty, to ensure consistent structure
        let skills_dir = output_dir.join("skills");
        self.ensure_dir(&skills_dir).await?;

        // Generate skills
        for skill in &config.skills {
            let skill_dir = skills_dir.join(&skill.name);
            let skill_path = skill_dir.join("SKILL.md");

            let content = self.generate_skill_md(skill);
            self.write_file(&skill_path, &content).await?;
            files.push(format!("skills/{}/SKILL.md", skill.name));
        }

        // Generate settings.json
        if config.mcp.is_some() {
            let settings_path = output_dir.join("settings.json");
            let settings_content = self.generate_settings(config);
            self.write_file(&settings_path, &settings_content).await?;
            files.push("settings.json".to_string());
        }

        // Generate mcp.json if MCP servers are defined
        if let Some(ref mcp) = config.mcp {
            if !mcp.servers.is_empty() {
                let mcp_path = output_dir.join("mcp.json");
                let mut servers = serde_json::Map::new();
                for server in &mcp.servers {
                    let mut server_config = serde_json::Map::new();
                    if let Some(ref cmd) = server.command {
                        server_config.insert(
                            "command".to_string(),
                            serde_json::Value::String(cmd.clone()),
                        );
                    }
                    if !server.args.is_empty() {
                        server_config.insert("args".to_string(), serde_json::json!(server.args));
                    }
                    servers.insert(
                        server.name.clone(),
                        serde_json::Value::Object(server_config),
                    );
                }
                let content =
                    serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": servers }))?;
                self.write_file(&mcp_path, &content).await?;
                files.push("mcp.json".to_string());
            }
        }

        Ok(BuildResult::success(
            Platform::Claude,
            output_dir.display().to_string(),
            files,
        ))
    }

    async fn install(&self, source: &Path, target: &InstallTarget) -> Result<()> {
        let dest = if target.global {
            self.global_install_path().ok_or_else(|| {
                HappyError::Other("Cannot determine global install path".to_string())
            })?
        } else {
            target
                .project_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".claude"))
        };

        // Ensure destination base exists
        self.ensure_dir(&dest).await?;

        // Copy skills directory
        let source_skills = source.join("skills");
        // Always try to copy if build was successful (source exists)
        if source_skills.exists() {
            let dest_skills = dest.join("skills");
            // Ensure dest_skills exists (explicitly, though copy_dir_all does it too)
            self.ensure_dir(&dest_skills).await?;
            copy_dir_all(&source_skills, &dest_skills).await?;
        }

        // Copy settings.json (merge if exists)
        let source_settings = source.join("settings.json");
        if source_settings.exists() {
            let dest_settings = dest.join("settings.json");
            tokio::fs::copy(&source_settings, &dest_settings).await?;
        }

        // Copy mcp.json
        let source_mcp = source.join("mcp.json");
        if source_mcp.exists() {
            let dest_mcp = dest.join("mcp.json");
            tokio::fs::copy(&source_mcp, &dest_mcp).await?;
        }

        Ok(())
    }

    fn validate(&self, config: &ProjectConfig) -> ValidationResult {
        let mut result = ValidationResult::ok();

        // Check for unsupported features
        if !config.workflows.is_empty() {
            // Claude supports workflows through hooks, but not native workflows
            result = result.with_warning(happy_core::ValidationWarning {
                field: "workflows".to_string(),
                message: "Claude doesn't have native workflow support".to_string(),
                suggestion: Some("Workflows will be converted to skill sequences".to_string()),
            });
        }

        result
    }

    async fn detect(&self) -> bool {
        // Check if claude CLI is available
        tokio::process::Command::new("claude")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn global_install_path(&self) -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".claude"))
    }
}

/// Helper to copy directory recursively
pub async fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            Box::pin(copy_dir_all(&path, &dest_path)).await?;
        } else {
            tokio::fs::copy(&path, &dest_path).await?;
        }
    }

    Ok(())
}
