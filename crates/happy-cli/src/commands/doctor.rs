//! Doctor command - Diagnose environment and dependencies

use anyhow::Result;
use colored::Colorize;
use happy_core::Platform;
use happy_adapters::create_adapter_factory;

pub async fn run() -> Result<()> {
    println!("{}", "🩺 Happy Coding Environment Diagnostic".cyan().bold());
    println!();

    // Check Rust version
    println!("{}", "🔧 System:".yellow());
    println!("  {} Rust: {}", "•", env!("CARGO_PKG_VERSION").cyan());
    println!("  {} OS: {}", "•", std::env::consts::OS.cyan());
    println!("  {} Arch: {}", "•", std::env::consts::ARCH.cyan());
    println!();

    // Check for configuration file
    println!("{}", "📁 Project:".yellow());
    let project_dir = std::env::current_dir()?;
    let config_exists = happy_core::ConfigManager::find_config_file(&project_dir);
    
    if let Some(config_path) = config_exists {
        println!("  {} Config: {}", "✅".green(), config_path.display());
    } else {
        println!("  {} Config: not found (run 'happy init' to create)", "⚠️".yellow());
    }
    println!();

    // Check platform tools
    println!("{}", "🎯 Platform Tools:".yellow());
    let adapter_factory = create_adapter_factory();

    for platform in Platform::all() {
        if let Some(adapter) = adapter_factory.get(*platform) {
            let available = adapter.detect().await;
            let icon = if available { "✅".green() } else { "⚠️".yellow() };
            let status = if available { "available".green() } else { "not detected".yellow() };
            
            println!("  {} {}: {}", icon, platform.as_str().cyan(), status);
            
            // Show global path
            if let Some(global_path) = adapter.global_install_path() {
                let exists = global_path.exists();
                let path_status = if exists { "exists".dimmed() } else { "not found".dimmed() };
                println!("      Global: {} ({})", global_path.display().to_string().dimmed(), path_status);
            }

            // Show limitations
            if !available {
                let limitations = adapter.limitations();
                if !limitations.is_empty() {
                    println!("      Note: {}", limitations[0].dimmed());
                }
            }
        }
    }
    println!();

    // Check common dependencies
    println!("{}", "📦 Dependencies:".yellow());
    
    let deps = [
        ("node", "Node.js"),
        ("npm", "npm"),
        ("git", "Git"),
    ];

    for (cmd, name) in deps {
        let available = tokio::process::Command::new(cmd)
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        let icon = if available { "✅".green() } else { "⚠️".yellow() };
        let status = if available { "available".green() } else { "not found".yellow() };
        
        println!("  {} {}: {}", icon, name, status);
    }
    println!();

    // Summary
    println!("{}", "📋 Summary:".yellow());
    println!("  Happy Coding is ready to use!");
    println!("  Run {} to get started.", "happy init".cyan());

    Ok(())
}
