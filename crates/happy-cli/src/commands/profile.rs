//! Profile management commands

use crate::config::SettingsManager;
use anyhow::Result;
use colored::Colorize;

pub async fn list() -> Result<()> {
    let settings = SettingsManager::load()?;

    println!("{}", "🔧 AI Profiles".blue().bold());
    println!();

    if settings.profiles.is_empty() {
        println!("   (No profiles configured)");
        println!();
        println!("   Create one with: {}", "happy connect <vendor>".dimmed());
    } else {
        for profile in &settings.profiles {
            let is_active = settings.active_profile.as_ref() == Some(&profile.name);
            let marker = if is_active { "*" } else { " " };

            println!(
                "   [{}] {} - {:?}",
                marker,
                profile.name.cyan(),
                profile.provider
            );

            if let Some(ref model) = profile.model {
                println!("       Model: {}", model.dimmed());
            }
            if let Some(ref base_url) = profile.base_url {
                println!("       URL: {}", base_url.dimmed());
            }
        }
    }

    Ok(())
}

pub async fn add(_name: &str) -> Result<()> {
    println!("{}", "Use 'happy connect <vendor>' to add a profile".yellow());
    Ok(())
}

pub async fn use_profile(name: &str) -> Result<()> {
    let mut settings = SettingsManager::load()?;

    if !settings.profiles.iter().any(|p| p.name == name) {
        anyhow::bail!("Profile '{}' not found", name);
    }

    settings.active_profile = Some(name.to_string());
    SettingsManager::save(&settings)?;

    println!("{}", format!("✅ Active profile set to '{}'", name).green());
    Ok(())
}

pub async fn remove(name: &str) -> Result<()> {
    let mut settings = SettingsManager::load()?;

    if !settings.profiles.iter().any(|p| p.name == name) {
        anyhow::bail!("Profile '{}' not found", name);
    }

    // Confirm deletion
    let confirm: bool = dialoguer::Confirm::new()
        .with_prompt(format!("Delete profile '{}'?", name))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{}", "Cancelled".dimmed());
        return Ok(());
    }

    settings.profiles.retain(|p| p.name != name);

    // Clear active profile if it was this one
    if settings.active_profile.as_ref() == Some(&name.to_string()) {
        settings.active_profile = settings.profiles.first().map(|p| p.name.clone());
    }

    SettingsManager::save(&settings)?;

    println!("{}", format!("✅ Profile '{}' deleted", name).green());
    Ok(())
}
