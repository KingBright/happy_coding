//! Run specific agent

use crate::commands::env;
use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;

pub async fn execute(agent: String, args: Vec<String>) -> Result<()> {
    match agent.to_lowercase().as_str() {
        "claude" => {
            // Delegate to env::run (which uses active environment or default)
            env::run(None, args).await?;
        }
        "codex" => {
            println!("{}", "🔹 Launching Codex...".blue());

            // Check if codex is installed
            if which::which("codex").is_err() {
                println!("{}", "⚠️ Codex CLI not found.".yellow());
                println!("Please ensure 'codex' is installed and in your PATH.");
                return Ok(());
            }

            let status = Command::new("codex")
                .args(&args)
                .status()
                .context("Failed to start Codex")?;

            if !status.success() {
                println!(
                    "{}",
                    format!("Codex exited with code: {:?}", status.code()).yellow()
                );
            }
        }
        "antigravity" => {
            println!("{}", "🔹 Antigravity Agent".blue());
            println!("Antigravity usually works as an IDE plugin or background service.");
            println!("To use it, please ensure the Google Antigravity extension is installed in your IDE.");
            println!("\nConfiguration location: ~/.gemini/antigravity/");

            // Helpful check
            if dirs::home_dir()
                .map(|h| h.join(".gemini/antigravity"))
                .unwrap_or_default()
                .exists()
            {
                println!("{}", "✓ Configuration detected.".green());
            } else {
                println!(
                    "{}",
                    "⚠️ Configuration not found. Run 'happy build' to generate it.".yellow()
                );
            }
        }
        other => {
            println!("{} Unknown agent '{}'", "Error:".red(), other);
            println!("Supported agents: claude, codex, antigravity");
        }
    }

    Ok(())
}
