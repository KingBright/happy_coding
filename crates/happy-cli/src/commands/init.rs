//! Init command - Initialize a new Happy Coding project

use std::path::Path;
use anyhow::Result;
use colored::Colorize;
use happy_core::{ConfigManager, ProjectConfig, TargetConfig, TargetsConfig};

pub async fn run(name: &str, skip_prompts: bool) -> Result<()> {
    println!("{}", "🚀 Initializing Happy Coding project...".cyan().bold());

    let project_dir = if name == "." {
        std::env::current_dir()?
    } else {
        let dir = std::env::current_dir()?.join(name);
        tokio::fs::create_dir_all(&dir).await?;
        dir
    };

    let project_name = if name == "." {
        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-project")
            .to_lowercase()
            .replace(' ', "-")
    } else {
        name.to_lowercase().replace(' ', "-")
    };

    // Check if config already exists
    let config_path = project_dir.join("happy.config.yaml");
    if config_path.exists() && !skip_prompts {
        println!("{} Configuration file already exists", "⚠️".yellow());
        return Ok(());
    }

    // Create default configuration
    let config = ProjectConfig {
        name: project_name.clone(),
        version: "1.0.0".to_string(),
        description: Some(format!("Happy Coding project: {}", project_name)),
        author: None,
        targets: TargetsConfig {
            claude: Some(TargetConfig::default()),
            codex: Some(TargetConfig::default()),
            antigravity: None,
        },
        skills: vec![],
        workflows: vec![],
        commands: vec![],
        mcp: None,
    };

    // Save configuration
    let config_manager = ConfigManager::new();
    config_manager.save(&config, &config_path)?;

    // Create directory structure
    let dirs = [
        "skills",
        "workflows",
        "commands",
    ];

    for dir in dirs {
        let dir_path = project_dir.join(dir);
        tokio::fs::create_dir_all(&dir_path).await?;
        
        // Create .gitkeep
        let gitkeep = dir_path.join(".gitkeep");
        tokio::fs::write(&gitkeep, "").await?;
    }

    // Create example skill
    let example_skill_dir = project_dir.join("skills/hello-world");
    tokio::fs::create_dir_all(&example_skill_dir).await?;
    
    let example_skill = r#"---
name: hello-world
description: A simple hello world skill example
---

When this skill is invoked, greet the user warmly and ask how you can help them today.

## Guidelines

1. Be friendly and professional
2. Offer relevant suggestions based on context
3. Use markdown formatting when appropriate
"#;

    tokio::fs::write(example_skill_dir.join("SKILL.md"), example_skill).await?;

    println!("{}", "✅ Project initialized successfully!".green().bold());
    println!();
    println!("Created files:");
    println!("  {} - Project configuration", "happy.config.yaml".cyan());
    println!("  {} - Example skill", "skills/hello-world/SKILL.md".cyan());
    println!();
    println!("Next steps:");
    println!("  1. Edit {} to configure your project", "happy.config.yaml".cyan());
    println!("  2. Add more skills in {}", "skills/".cyan());
    println!("  3. Run {} to build for all platforms", "happy build".cyan());

    Ok(())
}
