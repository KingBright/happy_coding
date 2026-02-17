//! Validate command - Validate configuration

use anyhow::Result;
use colored::Colorize;
use happy_core::{ConfigManager, Builder};
use happy_adapters::create_adapter_factory;

pub async fn run() -> Result<()> {
    println!("{}", "🔍 Validating Happy Coding configuration...".cyan().bold());

    let project_dir = std::env::current_dir()?;
    
    // Load configuration
    let mut config_manager = ConfigManager::new();
    let (config, config_path) = config_manager.load_from_directory(&project_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    println!("  📁 Config file: {}", config_path.display().to_string().dimmed());
    println!();

    // Validate configuration schema
    println!("{}", "📋 Configuration validation:".yellow());
    let schema_result = config_manager.validate(&config);
    
    let mut has_errors = false;

    if schema_result.valid {
        println!("  {} Schema is valid", "✅".green());
    } else {
        has_errors = true;
        println!("  {} Schema validation failed", "❌".red());
        for error in &schema_result.errors {
            println!("      {} {}: {}", "•".red(), error.field.red(), error.message);
        }
    }

    for warning in &schema_result.warnings {
        println!("  {} {}: {}", "⚠️".yellow(), warning.field.yellow(), warning.message);
        if let Some(ref suggestion) = warning.suggestion {
            println!("      💡 {}", suggestion.dimmed());
        }
    }

    println!();

    // Validate per-platform
    println!("{}", "🎯 Platform validation:".yellow());
    let adapter_factory = create_adapter_factory();
    let builder = Builder::new(adapter_factory);
    let platform_results = builder.validate(&config);

    for (platform, result) in platform_results {
        if result.valid && result.errors.is_empty() && result.warnings.is_empty() {
            println!("  {} {}", "✅".green(), platform.as_str().cyan());
        } else {
            if !result.valid {
                has_errors = true;
                println!("  {} {}", "❌".red(), platform.as_str().red());
            } else {
                println!("  {} {}", "⚠️".yellow(), platform.as_str().yellow());
            }

            for error in &result.errors {
                println!("      {} {}: {}", "•".red(), error.field, error.message);
            }
            for warning in &result.warnings {
                println!("      {} {}: {}", "•".yellow(), warning.field, warning.message);
                if let Some(ref suggestion) = warning.suggestion {
                    println!("        💡 {}", suggestion.dimmed());
                }
            }
        }
    }

    println!();

    // Summary
    if has_errors {
        println!("{}", "❌ Validation failed - please fix the errors above".red().bold());
        return Err(anyhow::anyhow!("Validation failed"));
    } else {
        println!("{}", "✅ Configuration is valid!".green().bold());
    }

    Ok(())
}
