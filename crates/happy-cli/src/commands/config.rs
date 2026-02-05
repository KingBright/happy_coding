//! Configuration sync commands
//!
//! Syncs Claude settings between local project and ~/.claude/

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

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

/// Push local config to ~/.claude/ (Merge mode with Backup)
pub async fn push() -> Result<()> {
    let local = local_config_path();
    let system = claude_config_path();

    if !local.exists() {
        return Err(anyhow::anyhow!("本地配置文件不存在: {}", local.display()));
    }

    // Read Local
    let local_content = std::fs::read_to_string(&local)?;
    let local_json: serde_json::Value = serde_json::from_str(&local_content)
        .map_err(|e| anyhow::anyhow!("本地配置文件 JSON 格式无效: {}", e))?;

    // Read System (if exists)
    let system_exists = system.exists();
    let mut system_json = if system_exists {
        let system_content = std::fs::read_to_string(&system)?;
        serde_json::from_str(&system_content)
            .map_err(|e| anyhow::anyhow!("系统配置文件 JSON 格式无效: {}", e))?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    // Calculate changes for preview
    let original_json = system_json.clone();

    // Merge: Local overrides System
    merge_json(&mut system_json, &local_json);

    // Show Diff
    if system_exists {
        print_json_diff(&original_json, &system_json);
    } else {
        println!("{}", "✨ 将创建新的系统配置文件".green());
    }

    if let Some(parent) = system.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Backup Original
    if system_exists {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_path = system.with_extension(format!("json.{}.bak", timestamp));
        std::fs::copy(&system, &backup_path)?;
        println!(
            "📦 已备份原文件: {}",
            backup_path.display().to_string().dimmed()
        );
    }

    // Write back pretty printed
    let new_system_content = serde_json::to_string_pretty(&system_json)?;
    std::fs::write(&system, new_system_content)?;

    println!("{}", "✓ 已将本地配置合并推送到 Claude Code".green());
    println!("  {} -> {} (Merge)", local.display(), system.display());

    Ok(())
}

fn print_json_diff(old: &serde_json::Value, new: &serde_json::Value) {
    println!("{}", "变更预览:".yellow().bold());

    let old_str = serde_json::to_string_pretty(old).unwrap_or_default();
    let new_str = serde_json::to_string_pretty(new).unwrap_or_default();

    let old_lines: Vec<&str> = old_str.lines().collect();
    let new_lines: Vec<&str> = new_str.lines().collect();

    // Simple verification of added lines
    for line in new_lines {
        // Simple logic to show Added lines (not perfect but helpful)
        if !old_lines.contains(&line) {
            println!("{} {}", "+".green(), line.green());
        }
    }
}

fn merge_json(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(ref mut old_map), serde_json::Value::Object(ref new_map)) => {
            for (k, v) in new_map {
                merge_json(
                    old_map.entry(k.clone()).or_insert(serde_json::Value::Null),
                    v,
                );
            }
        }
        (target, source) => {
            *target = source.clone();
        }
    }
}

/// Pull config from ~/.claude/ to local
pub async fn pull() -> Result<()> {
    let local = local_config_path();
    let system = claude_config_path();

    if !system.exists() {
        return Err(anyhow::anyhow!(
            "Claude 配置文件不存在: {}",
            system.display()
        ));
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
