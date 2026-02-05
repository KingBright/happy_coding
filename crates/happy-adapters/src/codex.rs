//! OpenAI Codex adapter
//!
//! Generates configuration for OpenAI Codex CLI:
//! - `.codex/skills/<name>/SKILL.md` - Skill definitions
//! - `.codex/AGENTS.md` - Agent instructions
//! - `.codex/config.toml` - Codex configuration

use async_trait::async_trait;
use happy_core::{
    Adapter, BuildResult, Feature, HappyError, InstallTarget, Platform, ProjectConfig, Result,
    SkillDefinition, ValidationResult, WorkflowDefinition,
};
use std::path::{Path, PathBuf};

/// OpenAI Codex platform adapter
pub struct CodexAdapter;

impl CodexAdapter {
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
        content.push_str("---\n\n");

        // Prompt content
        if let Some(ref prompt) = skill.prompt {
            content.push_str(prompt);
        }
        content.push('\n');

        // Parameters as requirements
        if !skill.parameters.is_empty() {
            content.push_str("\n## Requirements\n\n");
            for param in &skill.parameters {
                content.push_str(&format!("- {}: {}\n", param.name, param.description));
            }
        }

        content
    }

    /// Generate AGENTS.md content
    fn generate_agents_md(&self, config: &ProjectConfig) -> String {
        let mut content = String::new();

        content.push_str(&format!("# {} Agent Instructions\n\n", config.name));

        if let Some(ref desc) = config.description {
            content.push_str(&format!("{}\n\n", desc));
        }

        content.push_str("## Working Agreements\n\n");

        // Add skill summaries
        if !config.skills.is_empty() {
            content.push_str("### Available Skills\n\n");
            for skill in &config.skills {
                content.push_str(&format!("- `${}`: {}\n", skill.name, skill.description));
            }
            content.push('\n');
        }

        // Add workflow summaries
        if !config.workflows.is_empty() {
            content.push_str("### Workflows\n\n");
            for workflow in &config.workflows {
                content.push_str(&format!(
                    "- **{}**: {}\n",
                    workflow.name, workflow.description
                ));
            }
            content.push('\n');
        }

        content
    }

    /// Generate config.toml content
    fn generate_config_toml(&self, config: &ProjectConfig) -> String {
        let mut content = String::new();

        content.push_str("#:schema https://developers.openai.com/codex/config-schema.json\n\n");
        content.push_str("# Core settings\n");
        content.push_str("model = \"gpt-5.2-codex\"\n");
        content.push_str("approval_policy = \"on-request\"\n");
        content.push_str("sandbox_mode = \"workspace-write\"\n\n");

        // MCP servers
        if let Some(ref mcp) = config.mcp {
            if !mcp.servers.is_empty() {
                content.push_str("# MCP Servers\n");
                for server in &mcp.servers {
                    content.push_str(&format!("[mcp_servers.{}]\n", server.name));
                    if let Some(ref cmd) = server.command {
                        content.push_str(&format!("command = \"{}\"\n", cmd));
                    }
                    if !server.args.is_empty() {
                        let args: Vec<String> =
                            server.args.iter().map(|a| format!("\"{}\"", a)).collect();
                        content.push_str(&format!("args = [{}]\n", args.join(", ")));
                    }
                    if let Some(ref url) = server.url {
                        content.push_str(&format!("url = \"{}\"\n", url));
                    }
                    content.push('\n');
                }
            }
        }

        content
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

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Adapter for CodexAdapter {
    fn platform(&self) -> Platform {
        Platform::Codex
    }

    fn supported_features(&self) -> &[Feature] {
        &[Feature::Skill, Feature::Mcp, Feature::Rules]
    }

    fn limitations(&self) -> &[&str] {
        &[
            "Custom slash commands not supported",
            "Workflows must be simulated through AGENTS.md",
            "GPT-5 models may reject custom instructions",
        ]
    }

    async fn build(&self, config: &ProjectConfig, output_dir: &Path) -> Result<BuildResult> {
        let mut files = Vec::new();

        // Ensure output directory exists
        self.ensure_dir(output_dir).await?;

        // Generate skills
        let skills_dir = output_dir.join("skills");
        for skill in &config.skills {
            let skill_dir = skills_dir.join(&skill.name);
            let skill_path = skill_dir.join("SKILL.md");

            let content = self.generate_skill_md(skill);
            self.write_file(&skill_path, &content).await?;
            files.push(format!("skills/{}/SKILL.md", skill.name));
        }

        // Generate AGENTS.md
        let agents_path = output_dir.join("AGENTS.md");
        let agents_content = self.generate_agents_md(config);
        self.write_file(&agents_path, &agents_content).await?;
        files.push("AGENTS.md".to_string());

        // Generate config.toml
        let config_path = output_dir.join("config.toml");
        let config_content = self.generate_config_toml(config);
        self.write_file(&config_path, &config_content).await?;
        files.push("config.toml".to_string());

        Ok(BuildResult::success(
            Platform::Codex,
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
                .unwrap_or_else(|| PathBuf::from(".codex"))
        };

        // Copy skills directory
        let source_skills = source.join("skills");
        if source_skills.exists() {
            let dest_skills = dest.join("skills");
            crate::claude::copy_dir_all(&source_skills, &dest_skills).await?;
        }

        // Copy AGENTS.md
        let source_agents = source.join("AGENTS.md");
        if source_agents.exists() {
            let dest_agents = dest.join("AGENTS.md");
            tokio::fs::copy(&source_agents, &dest_agents).await?;
        }

        // Merge config.toml
        let source_config = source.join("config.toml");
        if source_config.exists() {
            let dest_config = dest.join("config.toml");
            tokio::fs::copy(&source_config, &dest_config).await?;
        }

        Ok(())
    }

    fn validate(&self, config: &ProjectConfig) -> ValidationResult {
        let mut result = ValidationResult::ok();

        // Codex doesn't support custom commands
        if !config.commands.is_empty() {
            result = result.with_warning(happy_core::ValidationWarning {
                field: "commands".to_string(),
                message: "Codex doesn't support custom slash commands".to_string(),
                suggestion: Some("Commands will be ignored for Codex platform".to_string()),
            });
        }

        result
    }

    async fn detect(&self) -> bool {
        // Check if codex CLI is available
        tokio::process::Command::new("codex")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn global_install_path(&self) -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".codex"))
    }
}
