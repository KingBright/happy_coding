//! Authentication commands

use crate::api::Client;
use crate::config::SettingsManager;
use anyhow::{Context, Result};
use colored::Colorize;

pub async fn login_interactive() -> Result<()> {
    println!("{}", "🔹 Login to Happy Remote".blue().bold());
    println!();

    // Get email
    let email: String = dialoguer::Input::new()
        .with_prompt("Email")
        .interact_text()?;

    // Get password
    let password: String = dialoguer::Password::new()
        .with_prompt("Password")
        .interact()?;

    println!();
    println!("{}", "🔐 Authenticating...".dimmed());

    do_login(&email, &password).await
}

pub async fn login_non_interactive(email: &str, password: &str) -> Result<()> {
    println!("{}", "🔹 Login to Happy Remote".blue().bold());
    println!();
    println!("   Email: {}", email.dimmed());
    println!("   Password: {}", "********".dimmed());
    println!();
    println!("{}", "🔐 Authenticating...".dimmed());

    do_login(email, password).await
}

async fn do_login(email: &str, password: &str) -> Result<()> {

    // Call API
    let client = Client::new();
    let result = client
        .login(&email, &password)
        .await;

    let tokens = match result {
        Ok(tokens) => tokens,
        Err(e) => {
            // Check if error is 401 (unauthorized) - might be invalid credentials
            let error_msg = e.to_string();
            if error_msg.contains("401") || error_msg.contains("Unauthorized") {
                anyhow::bail!("Invalid email or password");
            }
            return Err(e).context("Login failed");
        }
    };

    // Save tokens and credentials for auto-login
    let mut settings = SettingsManager::load()?;
    settings.access_token = Some(tokens.access_token.clone());
    settings.refresh_token = Some(tokens.refresh_token);
    settings.email = Some(email.to_string());
    settings.password = Some(password.to_string());
    SettingsManager::save(&settings)?;

    // Get user info
    match client.get_user_info(&tokens.access_token).await {
        Ok(user) => {
            settings.user_id = Some(user.id.clone());
            SettingsManager::save(&settings)?;

            println!();
            println!("{}", "✅ Login successful!".green().bold());
            println!();
            println!("   Welcome, {}!", user.name.as_deref().unwrap_or(&user.email).cyan());
        }
        Err(_) => {
            println!();
            println!("{}", "✅ Login successful!".green().bold());
        }
    }

    Ok(())
}

pub async fn logout() -> Result<()> {
    let mut settings = SettingsManager::load()?;

    if settings.access_token.is_none() {
        println!("{}", "⚠️  Not logged in".yellow());
        return Ok(());
    }

    // Optionally revoke token on server
    if let Some(ref token) = settings.access_token {
        let client = Client::new();
        let _ = client.logout(token).await;
    }

    // Clear local tokens and credentials
    settings.access_token = None;
    settings.refresh_token = None;
    settings.user_id = None;
    settings.email = None;
    settings.password = None;
    SettingsManager::save(&settings)?;

    println!("{}", "✅ Logged out successfully".green());
    Ok(())
}

pub async fn whoami() -> Result<()> {
    let settings = SettingsManager::load()?;

    if settings.access_token.is_none() {
        println!("{}", "⚠️  Not logged in".yellow());
        return Ok(());
    }

    let client = Client::new();
    let token = settings.access_token.unwrap();

    match client.get_user_info(&token).await {
        Ok(user) => {
            println!("{}", "👤 User Info".blue().bold());
            println!();
            println!("   ID:    {}", user.id.dimmed());
            println!("   Email: {}", user.email.cyan());
            if let Some(name) = user.name {
                println!("   Name:  {}", name);
            }
        }
        Err(e) => {
            println!("{}", format!("⚠️  Failed to get user info: {}", e).yellow());
        }
    }

    Ok(())
}

pub async fn keys() -> Result<()> {
    let settings = SettingsManager::load()?;

    if settings.access_token.is_none() {
        println!("{}", "⚠️  Not logged in".yellow());
        return Ok(());
    }

    let client = Client::new();
    let token = settings.access_token.unwrap();

    match client.list_access_keys(&token).await {
        Ok(keys) => {
            println!("{}", "🔑 Access Keys".blue().bold());
            println!();

            if keys.is_empty() {
                println!("   (No access keys)");
            } else {
                for key in keys {
                    println!(
                        "   {} {} - {} ({})",
                        if key.is_revoked { "❌" } else { "✓" },
                        key.name.cyan(),
                        key.key_prefix,
                        key.created_at.format("%Y-%m-%d")
                    );
                }
            }
        }
        Err(e) => {
            println!("{}", format!("⚠️  Failed to list keys: {}", e).yellow());
        }
    }

    Ok(())
}
