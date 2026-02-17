//! Environment management commands
//!
//! Manages Claude Code and Codex CLI environments.

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

const API_TIMEOUT_MS: u64 = 3000000;

/// Claude settings.json structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeSettings {
    #[serde(default)]
    providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    active_provider: Option<String>,
    #[serde(default)]
    env: HashMap<String, serde_json::Value>,
    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderConfig {
    api_key: String,
    base_url: String,
    #[serde(default)]
    model: Option<String>,
}

fn claude_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/settings.json")
}

fn load_claude_settings() -> Result<ClaudeSettings> {
    let path = claude_config_path();
    if !path.exists() {
        return Ok(ClaudeSettings::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let settings: ClaudeSettings = serde_json::from_str(&content)?;
    Ok(settings)
}

fn save_claude_settings(settings: &ClaudeSettings) -> Result<()> {
    let path = claude_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, content)?;
    Ok(())
}

fn mask_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    } else {
        "****".to_string()
    }
}

/// List all environments
pub async fn list() -> Result<()> {
    println!("{}", "=== Claude Code Environments ===".cyan().bold());

    let settings = load_claude_settings()?;
    let active = settings.active_provider.as_deref().unwrap_or("");

    if settings.providers.is_empty() {
        println!("  {}", "(No environments configured)".dimmed());
    } else {
        for (name, config) in &settings.providers {
            let prefix = if name == active {
                "* ".green()
            } else {
                "  ".normal()
            };
            println!("{}{}", prefix, name.cyan());
            println!("    Type:    Claude Code");
            println!("    BaseUrl: {}", config.base_url.dimmed());
            println!("    ApiKey:  {}", mask_key(&config.api_key).dimmed());
            if let Some(ref model) = config.model {
                println!("    Model:   {}", model.dimmed());
            }
        }
    }

    Ok(())
}

/// Add a new environment
pub async fn add(name: &str) -> Result<()> {
    let mut settings = load_claude_settings()?;

    // Check if exists and get defaults
    let existing = settings.providers.get(name);
    let default_url = existing.map(|p| p.base_url.as_str()).unwrap_or("");
    let default_model = existing.and_then(|p| p.model.as_deref()).unwrap_or("");

    // Prompt for API key
    print!("🔑 请输入 API Key");
    if existing.is_some() {
        print!(" (回车跳过)");
    }
    print!(": ");
    io::stdout().flush()?;

    let api_key = {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() && existing.is_some() {
            existing.unwrap().api_key.clone()
        } else if input.is_empty() {
            return Err(anyhow::anyhow!("API Key 不能为空"));
        } else {
            input.to_string()
        }
    };

    // Prompt for base URL
    print!("🌐 请输入 Base URL");
    if !default_url.is_empty() {
        print!(" [{}]", default_url);
    }
    print!(": ");
    io::stdout().flush()?;

    let base_url = {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            default_url.to_string()
        } else {
            input.to_string()
        }
    };

    // Prompt for Default Model
    print!("🧠 请输入默认模型 (e.g. claude-3-opus-20240229)");
    if !default_model.is_empty() {
        print!(" [{}]", default_model);
    }
    print!(": ");
    io::stdout().flush()?;

    let model = {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            if default_model.is_empty() {
                None
            } else {
                Some(default_model.to_string())
            }
        } else {
            Some(input.to_string())
        }
    };

    // Update settings
    settings.providers.insert(
        name.to_string(),
        ProviderConfig {
            api_key: api_key.trim().to_string(),
            base_url: base_url.clone(),
            model,
        },
    );

    // Set as active if first provider
    if settings.active_provider.is_none() {
        settings.active_provider = Some(name.to_string());
    }

    // Update env with active provider
    update_env_from_active(&mut settings);

    save_claude_settings(&settings)?;

    println!(
        "{}",
        format!("✅ Claude environment '{}' added/updated.", name).green()
    );
    Ok(())
}

/// Switch to an environment
pub async fn switch(name: &str) -> Result<()> {
    let mut settings = load_claude_settings()?;

    if !settings.providers.contains_key(name) {
        return Err(anyhow::anyhow!("Environment '{}' not found", name));
    }

    settings.active_provider = Some(name.to_string());
    update_env_from_active(&mut settings);

    save_claude_settings(&settings)?;

    println!(
        "{}",
        format!("✅ Switched to environment: {}", name).green()
    );
    Ok(())
}

/// Delete an environment
pub async fn delete(name: &str) -> Result<()> {
    let mut settings = load_claude_settings()?;

    if settings.providers.remove(name).is_some() {
        // Clear active provider if it was the deleted one
        if settings.active_provider.as_deref() == Some(name) {
            settings.active_provider = settings.providers.keys().next().cloned();
            update_env_from_active(&mut settings);
        }
        save_claude_settings(&settings)?;
        println!("{}", format!("✅ Environment '{}' deleted.", name).green());
    } else {
        println!(
            "{}",
            format!("⚠️ Environment '{}' not found.", name).yellow()
        );
    }

    Ok(())
}

/// Run Claude with a specific environment
pub async fn run(name: Option<&str>, args: Vec<String>) -> Result<()> {
    let settings = load_claude_settings()?;

    let env_name = name
        .map(|s| s.to_string())
        .or(settings.active_provider.clone())
        .ok_or_else(|| anyhow::anyhow!("No environment specified and no default set"))?;

    let provider = settings
        .providers
        .get(&env_name)
        .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env_name))?;

    println!(
        "{}",
        format!("🔹 Starting Claude Code with environment: {}", env_name).blue()
    );

    let mut cmd = Command::new("claude");
    cmd.args(&args)
        .env("ANTHROPIC_AUTH_TOKEN", provider.api_key.trim())
        .env("ANTHROPIC_BASE_URL", &provider.base_url)
        .env("API_TIMEOUT_MS", API_TIMEOUT_MS.to_string())
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");

    if let Some(ref model) = provider.model {
        cmd.env("ANTHROPIC_MODEL", model);
        cmd.env("ANTHROPIC_DEFAULT_HAIKU_MODEL", model);
        cmd.env("ANTHROPIC_DEFAULT_SONNET_MODEL", model);
        cmd.env("ANTHROPIC_DEFAULT_OPUS_MODEL", model);
        println!("{}", format!("   Model: {}", model).dimmed());
    }

    let status = cmd.status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Claude exited with code: {:?}",
            status.code()
        ));
    }

    Ok(())
}

fn update_env_from_active(settings: &mut ClaudeSettings) {
    if let Some(ref name) = settings.active_provider {
        if let Some(provider) = settings.providers.get(name) {
            settings.env.insert(
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                serde_json::Value::String(provider.api_key.clone()),
            );
            settings.env.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                serde_json::Value::String(provider.base_url.clone()),
            );
            settings.env.insert(
                "API_TIMEOUT_MS".to_string(),
                serde_json::Value::Number(API_TIMEOUT_MS.into()),
            );
            settings.env.insert(
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_string(),
                serde_json::Value::Number(1.into()),
            );

            if let Some(ref model) = provider.model {
                settings.env.insert(
                    "ANTHROPIC_MODEL".to_string(),
                    serde_json::Value::String(model.clone()),
                );
                settings.env.insert(
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL".to_string(),
                    serde_json::Value::String(model.clone()),
                );
                settings.env.insert(
                    "ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(),
                    serde_json::Value::String(model.clone()),
                );
                settings.env.insert(
                    "ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(),
                    serde_json::Value::String(model.clone()),
                );
            } else {
                settings.env.remove("ANTHROPIC_MODEL");
                settings.env.remove("ANTHROPIC_DEFAULT_HAIKU_MODEL");
                settings.env.remove("ANTHROPIC_DEFAULT_SONNET_MODEL");
                settings.env.remove("ANTHROPIC_DEFAULT_OPUS_MODEL");
            }
        }
    }
}
