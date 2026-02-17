//! Google Antigravity adapter
//!
//! Generates configuration for Antigravity IDE:
//! - `.agent/skills/<name>.yaml` - Skill definitions
//! - `.agent/workflows/<name>.md` - Workflow definitions
//! - `.agent/rules/` - User rules

use async_trait::async_trait;
use happy_core::{
    Adapter, BuildResult, Feature, HappyError, InstallTarget, Platform, ProjectConfig, Result,
    SkillDefinition, ValidationResult, WorkflowDefinition,
};
use std::path::{Path, PathBuf};

/// Antigravity platform adapter
pub struct AntigravityAdapter;

impl AntigravityAdapter {
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

        content
    }

    /// Generate workflow markdown
    fn generate_workflow_md(&self, workflow: &WorkflowDefinition) -> String {
        let mut content = String::new();

        // Frontmatter
        content.push_str("---\n");
        content.push_str(&format!("description: {}\n", workflow.description));
        content.push_str("---\n\n");

        // Steps
        for (i, step) in workflow.steps.iter().enumerate() {
            content.push_str(&format!("## Step {}\n\n", i + 1));

            if let Some(ref skill) = step.skill {
                content.push_str(&format!(
                    "Use skill `${}` to complete this step.\n\n",
                    skill
                ));
            }
            if let Some(ref command) = step.command {
                content.push_str(&format!("Run command:\n```bash\n{}\n```\n\n", command));
            }
            if let Some(ref prompt) = step.prompt {
                content.push_str(&format!("{}\n\n", prompt));
            }
            if let Some(ref condition) = step.condition {
                content.push_str(&format!("> Condition: {}\n\n", condition));
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

impl Default for AntigravityAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Adapter for AntigravityAdapter {
    fn platform(&self) -> Platform {
        Platform::Antigravity
    }

    fn supported_features(&self) -> &[Feature] {
        &[
            Feature::Skill,
            Feature::Workflow,
            Feature::Rules,
            Feature::Mcp,
        ]
    }

    fn limitations(&self) -> &[&str] {
        &[
            "Early preview - may have stability issues",
            "Requires Google account",
            "Code sent to Google servers",
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

        // Generate workflows
        let workflows_dir = output_dir.join("workflows");
        for workflow in &config.workflows {
            let workflow_path = workflows_dir.join(format!("{}.md", workflow.name));

            let content = self.generate_workflow_md(workflow);
            self.write_file(&workflow_path, &content).await?;
            files.push(format!("workflows/{}.md", workflow.name));
        }

        // Generate rules (if any commands defined, convert to rules)
        if !config.commands.is_empty() {
            let rules_dir = output_dir.join("rules");
            self.ensure_dir(&rules_dir).await?;

            let mut rules_content = String::from("# Project Commands\n\n");
            for command in &config.commands {
                rules_content.push_str(&format!("## /{}\n\n", command.name));
                rules_content.push_str(&format!("{}\n\n", command.description));
                if let Some(ref workflow) = command.workflow {
                    rules_content.push_str(&format!(
                        "Use workflow `{}` for this command.\n\n",
                        workflow
                    ));
                }
                if let Some(ref skill) = command.skill {
                    rules_content
                        .push_str(&format!("Use skill `${}` for this command.\n\n", skill));
                }
            }

            let rules_path = rules_dir.join("commands.md");
            self.write_file(&rules_path, &rules_content).await?;
            files.push("rules/commands.md".to_string());
        }

        Ok(BuildResult::success(
            Platform::Antigravity,
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
                .unwrap_or_else(|| PathBuf::from(".agent"))
        };

        // Copy all directories
        for dir_name in &["skills", "workflows", "rules"] {
            let source_dir = source.join(dir_name);
            if source_dir.exists() {
                let dest_dir = dest.join(dir_name);
                crate::claude::copy_dir_all(&source_dir, &dest_dir).await?;
            }
        }

        Ok(())
    }

    fn validate(&self, config: &ProjectConfig) -> ValidationResult {
        ValidationResult::ok()
    }

    async fn detect(&self) -> bool {
        // Check for Antigravity settings directory
        if let Some(home) = dirs::home_dir() {
            home.join(".gemini/antigravity").exists()
        } else {
            false
        }
    }

    fn global_install_path(&self) -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".gemini/antigravity/skills"))
    }
}
