//! Configuration sync commands
//!
//! Syncs Claude settings between local project and ~/.claude/

use std::path::PathBuf;
use anyhow::Result;
use colored::Colorize;

fn local_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("claude_settings.json")
}

fn claude_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/settings.json")
}

/// Push local config to ~/.claude/
pub async fn push() -> Result<()> {
    let local = local_config_path();
    let system = claude_config_path();
    
    if !local.exists() {
        return Err(anyhow::anyhow!("本地配置文件不存在: {}", local.display()));
    }
    
    // Validate JSON
    let content = std::fs::read_to_string(&local)?;
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| anyhow::anyhow!("本地配置文件 JSON 格式无效: {}", e))?;
    
    if let Some(parent) = system.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&local, &system)?;
    
    println!("{}", "✓ 已将本地配置推送到 Claude Code".green());
    println!("  {} -> {}", local.display(), system.display());
    
    Ok(())
}

/// Pull config from ~/.claude/ to local
pub async fn pull() -> Result<()> {
    let local = local_config_path();
    let system = claude_config_path();
    
    if !system.exists() {
        return Err(anyhow::anyhow!("Claude 配置文件不存在: {}", system.display()));
    }
    
    std::fs::copy(&system, &local)?;
    
    println!("{}", "✓ 已从 Claude Code 拉取配置到本地".green());
    println!("  {} -> {}", system.display(), local.display());
    
    Ok(())
}

/// Show diff between local and system config
pub async fn diff() -> Result<()> {
    let local = local_config_path();
    let system = claude_config_path();
    
    if !local.exists() {
        println!("{}", "本地配置文件不存在".yellow());
        return Ok(());
    }
    
    if !system.exists() {
        println!("{}", "Claude 配置文件不存在".yellow());
        return Ok(());
    }
    
    let local_content = std::fs::read_to_string(&local)?;
    let system_content = std::fs::read_to_string(&system)?;
    
    if local_content == system_content {
        println!("{}", "✓ 配置文件一致，无差异".green());
    } else {
        println!("{}", "配置文件存在差异:".yellow());
        
        // Simple line-by-line diff
        let local_lines: Vec<&str> = local_content.lines().collect();
        let system_lines: Vec<&str> = system_content.lines().collect();
        
        let max_lines = local_lines.len().max(system_lines.len());
        for i in 0..max_lines {
            let local_line = local_lines.get(i).unwrap_or(&"");
            let system_line = system_lines.get(i).unwrap_or(&"");
            
            if local_line != system_line {
                if !local_line.is_empty() {
                    println!("{} {}", "- ".red(), local_line.red());
                }
                if !system_line.is_empty() {
                    println!("{} {}", "+ ".green(), system_line.green());
                }
            }
        }
    }
    
    Ok(())
}
