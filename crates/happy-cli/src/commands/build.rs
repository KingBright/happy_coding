//! Build command - Build for all configured platforms

use std::path::PathBuf;
use anyhow::Result;
use colored::Colorize;
use happy_core::{Builder, BuildOptions, ConfigManager, Platform};
use happy_adapters::create_adapter_factory;

pub async fn run(target: Option<String>, watch: bool, clean: bool) -> Result<()> {
    println!("{}", "🔨 Building Happy Coding project...".cyan().bold());

    let project_dir = std::env::current_dir()?;
    
    // Load configuration
    let mut config_manager = ConfigManager::new();
    let (config, config_path) = config_manager.load_from_directory(&project_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    println!("  📁 Using config: {}", config_path.display().to_string().dimmed());

    // Parse target platform
    let target_platform = if let Some(ref t) = target {
        Some(match t.to_lowercase().as_str() {
            "claude" => Platform::Claude,
            "codex" => Platform::Codex,
            "antigravity" => Platform::Antigravity,
            _ => return Err(anyhow::anyhow!("Unknown platform: {}", t)),
        })
    } else {
        None
    };

    // Create builder with adapters
    let adapter_factory = create_adapter_factory();
    let builder = Builder::new(adapter_factory);

    // Build options
    let options = BuildOptions {
        target: target_platform,
        watch,
        clean,
    };

    // Run build
    let summary = builder.build(&config, &project_dir, &options).await
        .map_err(|e| anyhow::anyhow!("Build failed: {}", e))?;

    // Print summary
    println!("{}", builder.format_summary(&summary));

    if summary.success {
        println!("{}", "✅ Build completed successfully!".green().bold());
        
        for result in &summary.results {
            println!("  {} {} → {}", 
                "📦".green(),
                result.platform.as_str().cyan(),
                result.output_path.dimmed()
            );
            for file in &result.files {
                println!("      {}", file.dimmed());
            }
        }
    } else {
        println!("{}", "❌ Build failed!".red().bold());
        
        for result in &summary.results {
            if !result.success {
                println!("  {} {}:", "❌".red(), result.platform.as_str().red());
                for error in &result.errors {
                    println!("      {}", error.red());
                }
            }
        }

        return Err(anyhow::anyhow!("Build failed"));
    }

    // Watch mode
    if watch {
        println!();
        println!("{}", "👀 Watching for changes... (Ctrl+C to stop)".yellow());
        
        // TODO: Implement watch mode using watcher
        tokio::signal::ctrl_c().await?;
    }

    Ok(())
}
