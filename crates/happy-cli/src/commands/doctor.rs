//! Doctor command - Diagnostics

use anyhow::Result;
use colored::Colorize;

pub async fn execute() -> Result<()> {
    println!("{}", "🔍 Happy Remote Diagnostics".blue().bold());
    println!();

    // Check OS
    println!("{}", "System:".cyan());
    println!("   OS: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!();

    // Check for claude
    println!("{}", "Dependencies:".cyan());
    check_binary("claude", "Claude Code CLI").await;
    check_binary("tmux", "Tmux (optional, for session persistence)").await;
    println!();

    // Check daemon
    println!("{}", "Daemon:".cyan());
    let daemon_manager = crate::daemon::DaemonManager::new();
    if daemon_manager.is_running().await {
        println!("   {}", "✓ Daemon is running".green());
    } else {
        println!("   {}", "✗ Daemon is not running".red());
        println!("      Run: {}", "happy daemon start".dimmed());
    }
    println!();

    // Check settings
    println!("{}", "Configuration:".cyan());
    match crate::config::SettingsManager::load() {
        Ok(settings) => {
            if settings.access_token.is_some() {
                println!("   {}", "✓ Authenticated".green());
            } else {
                println!("   {}", "✗ Not authenticated".red());
                println!("      Run: {}", "happy auth login".dimmed());
            }

            if !settings.profiles.is_empty() {
                println!("   {} {} AI profile(s) configured", "✓".green(), settings.profiles.len());
            } else {
                println!("   {}", "✗ No AI profiles configured".red());
                println!("      Run: {}", "happy connect anthropic".dimmed());
            }
        }
        Err(e) => {
            println!("   {} Failed to load settings: {}", "✗".red(), e);
        }
    }
    println!();

    println!("{}", "Done!".green().bold());

    Ok(())
}

async fn check_binary(name: &str, description: &str) {
    match which::which(name) {
        Ok(path) => {
            let path_str = path.display().to_string();
            println!("   {} {} - {}", "✓".green(), description, path_str.dimmed());
        }
        Err(_) => {
            println!("   {} {} - {}", "✗".red(), description, "not found".red());
        }
    }
}
