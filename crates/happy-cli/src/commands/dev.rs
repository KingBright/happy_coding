//! Dev command - Start development mode with file watching

use anyhow::Result;
use colored::Colorize;
use happy_core::{Builder, BuildOptions, ConfigManager, Platform, watcher::{Watcher, WatchEvent, is_config_file}};
use happy_adapters::create_adapter_factory;

pub async fn run(target: Option<String>) -> Result<()> {
    println!("{}", "🔧 Starting development mode...".cyan().bold());

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

    // Initial build
    let adapter_factory = create_adapter_factory();
    let builder = Builder::new(adapter_factory);

    let options = BuildOptions {
        target: target_platform,
        watch: false,
        clean: false,
    };

    println!("{}", "📦 Running initial build...".yellow());
    let summary = builder.build(&config, &project_dir, &options).await
        .map_err(|e| anyhow::anyhow!("Build failed: {}", e))?;

    if summary.success {
        println!("{}", "✅ Initial build completed!".green());
    } else {
        println!("{}", "⚠️ Initial build had errors".yellow());
    }

    // Start file watcher
    println!();
    println!("{}", "👀 Watching for changes... (Ctrl+C to stop)".cyan().bold());

    let mut watcher = Watcher::new().with_debounce(500);
    watcher.watch(&project_dir)
        .map_err(|e| anyhow::anyhow!("Failed to start watcher: {}", e))?;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!();
                println!("{}", "👋 Stopping development mode...".yellow());
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                if let Some(event) = watcher.try_next_event() {
                    match event {
                        WatchEvent::Changed(path) | WatchEvent::Created(path) => {
                            // Skip build outputs
                            let path_str = path.display().to_string();
                            if path_str.contains(".claude/") 
                                || path_str.contains(".codex/")
                                || path_str.contains(".agent/")
                                || path_str.contains(".idx/") {
                                continue;
                            }

                            println!();
                            println!("{} {}", "📝 Changed:".yellow(), path.display());
                            
                            // Reload config if needed
                            if is_config_file(&path) {
                                config_manager.clear_cache();
                            }

                            // Rebuild
                            let adapter_factory = create_adapter_factory();
                            let builder = Builder::new(adapter_factory);
                            
                            match config_manager.load_from_directory(&project_dir) {
                                Ok((new_config, _)) => {
                                    match builder.build(&new_config, &project_dir, &options).await {
                                        Ok(summary) => {
                                            if summary.success {
                                                println!("{}", "✅ Rebuild completed!".green());
                                            } else {
                                                println!("{}", "⚠️ Rebuild had errors".yellow());
                                            }
                                        }
                                        Err(e) => {
                                            println!("{} {}", "❌ Rebuild failed:".red(), e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("{} {}", "❌ Config error:".red(), e);
                                }
                            }
                        }
                        WatchEvent::Removed(path) => {
                            println!("{} {}", "🗑️ Removed:".yellow(), path.display());
                        }
                        WatchEvent::Error(msg) => {
                            println!("{} {}", "⚠️ Watch error:".yellow(), msg);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
