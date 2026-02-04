//! Install command - Install built artifacts to target location

use anyhow::Result;
use colored::Colorize;
use happy_core::{ConfigManager, InstallTarget, Platform};
use happy_adapters::create_adapter_factory;

pub async fn run(global: bool, target: Option<String>) -> Result<()> {
    println!("{}", "📦 Installing Happy Coding artifacts...".cyan().bold());

    let project_dir = std::env::current_dir()?;
    
    // Load configuration
    let mut config_manager = ConfigManager::new();
    let (config, _config_path) = config_manager.load_from_directory(&project_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    // Determine platforms to install
    let platforms: Vec<Platform> = if let Some(ref t) = target {
        vec![match t.to_lowercase().as_str() {
            "claude" => Platform::Claude,
            "codex" => Platform::Codex,
            "antigravity" => Platform::Antigravity,
            _ => return Err(anyhow::anyhow!("Unknown platform: {}", t)),
        }]
    } else {
        config.targets.enabled_platforms()
    };

    let adapter_factory = create_adapter_factory();

    for platform in platforms {
        let output_dir = project_dir.join(config.output_dir(platform));
        
        if !output_dir.exists() {
            println!("  {} {} - not built, skipping", 
                "⚠️".yellow(), 
                platform.as_str().yellow()
            );
            continue;
        }

        let install_target = InstallTarget {
            platform,
            global,
            project_path: if global { None } else { Some(".".to_string()) },
        };

        if let Some(adapter) = adapter_factory.get(platform) {
            match adapter.install(&output_dir, &install_target).await {
                Ok(()) => {
                    let dest = if global {
                        adapter.global_install_path()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "global".to_string())
                    } else {
                        platform.default_output_dir().to_string()
                    };
                    println!("  {} {} → {}", 
                        "✅".green(), 
                        platform.as_str().cyan(),
                        dest.dimmed()
                    );
                }
                Err(e) => {
                    println!("  {} {} - {}", 
                        "❌".red(), 
                        platform.as_str().red(),
                        e
                    );
                }
            }
        } else {
            println!("  {} {} - adapter not found", 
                "❌".red(), 
                platform.as_str().red()
            );
        }
    }

    println!();
    println!("{}", "✅ Installation complete!".green().bold());

    Ok(())
}
