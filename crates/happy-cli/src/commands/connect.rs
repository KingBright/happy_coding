//! Connect to AI vendors

use crate::config::SettingsManager;
use anyhow::Result;
use colored::Colorize;
use happy_core::{AIProfile, AIProvider};

pub async fn execute(vendor: &str) -> Result<()> {
    println!("{}", format!("🔹 Connect to {}", vendor).blue().bold());
    println!();

    let provider = match vendor.to_lowercase().as_str() {
        "anthropic" | "claude" => AIProvider::Anthropic,
        "openai" => AIProvider::OpenAI,
        "azure" => AIProvider::Azure,
        "gemini" => AIProvider::Gemini,
        _ => anyhow::bail!(
            "Unknown vendor: {}. Supported: anthropic, openai, azure, gemini",
            vendor
        ),
    };

    // Get profile name
    let name: String = dialoguer::Input::new()
        .with_prompt("Profile name")
        .default(format!("{}-default", vendor.to_lowercase()))
        .interact_text()?;

    // Get API key
    let api_key: String = dialoguer::Password::new()
        .with_prompt("API Key")
        .interact()?;

    // Get optional base URL
    let base_url: String = dialoguer::Input::new()
        .with_prompt("Base URL (optional)")
        .allow_empty(true)
        .interact_text()?;

    // Get optional model
    let model: String = dialoguer::Input::new()
        .with_prompt("Model (optional)")
        .allow_empty(true)
        .interact_text()?;

    // Create profile
    let profile = AIProfile {
        name: name.clone(),
        provider,
        api_key: Some(api_key),
        base_url: if base_url.is_empty() {
            None
        } else {
            Some(base_url)
        },
        model: if model.is_empty() { None } else { Some(model) },
        default: false,
        env_vars: std::collections::HashMap::new(),
    };

    // Save profile
    let mut settings = SettingsManager::load()?;

    // Remove existing profile with same name
    settings.profiles.retain(|p| p.name != name);
    settings.profiles.push(profile);

    // Set as active if first profile
    if settings.active_profile.is_none() {
        settings.active_profile = Some(name.clone());
    }

    SettingsManager::save(&settings)?;

    println!();
    println!(
        "{}",
        format!("✅ Profile '{}' created successfully!", name)
            .green()
            .bold()
    );
    println!();
    println!(
        "   Use it with: {} {}",
        "happy run --profile".dimmed(),
        name.dimmed()
    );

    Ok(())
}
