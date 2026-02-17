//! Config command - Manage CLI configuration

use crate::config::SettingsManager;
use anyhow::{Context, Result};
use colored::Colorize;

/// Set the remote server URL
pub async fn set_server(url: &str) -> Result<()> {
    let mut settings = SettingsManager::load().context("Failed to load settings")?;

    // Validate URL format
    let url = url.trim().trim_end_matches('/');
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!(
            "Invalid URL: {}. URL must start with http:// or https://",
            url
        );
    }

    settings.server_url = format!("{}/api/v1", url);
    settings.webapp_url = url.to_string();

    SettingsManager::save(&settings).context("Failed to save settings")?;

    println!("{} Server URL set to: {}", "✓".green(), url.cyan());
    println!("  API endpoint: {}", settings.server_url.dimmed());
    println!("  Web app: {}", settings.webapp_url.dimmed());

    Ok(())
}

/// Set the daemon WebSocket port
pub async fn set_daemon_port(port: u16) -> Result<()> {
    // The daemon port is stored in a separate config file
    // since it needs to be accessible before settings are loaded
    let happy_home = SettingsManager::happy_home()?;
    let port_file = happy_home.join("daemon.port");

    // Ensure directory exists
    tokio::fs::create_dir_all(&happy_home)
        .await
        .context("Failed to create config directory")?;

    // Write port to file
    tokio::fs::write(&port_file, port.to_string())
        .await
        .context("Failed to write port file")?;

    println!(
        "{} Daemon WebSocket port set to: {}",
        "✓".green(),
        port.to_string().cyan()
    );
    println!("  Config file: {}", port_file.display().to_string().dimmed());
    println!(
        "{}",
        "  Note: Restart the daemon for changes to take effect."
            .yellow()
            .dimmed()
    );

    Ok(())
}

/// Show current configuration
pub async fn show() -> Result<()> {
    let settings = SettingsManager::load().context("Failed to load settings")?;

    println!("{}", "Happy Remote Configuration".bold().underline());
    println!();

    // Server configuration
    println!("{}", "Server Settings:".cyan().bold());
    println!("  API URL:    {}", settings.server_url);
    println!("  WebApp URL: {}", settings.webapp_url);
    println!();

    // Daemon port
    let daemon_port = get_daemon_port().await;
    println!("{}", "Daemon Settings:".cyan().bold());
    println!("  WebSocket Port: {}", daemon_port.to_string().cyan());
    println!();

    // User info
    println!("{}", "User:".cyan().bold());
    if let Some(user_id) = &settings.user_id {
        println!("  ID:    {}", user_id);
    } else {
        println!("  {}", "Not logged in".yellow());
    }
    if let Some(email) = &settings.email {
        println!("  Email: {}", email);
        if settings.password.is_some() {
            println!("  Saved credentials: {}", "Yes".green());
        }
    }
    println!();

    // Profiles
    println!("{}", "AI Profiles:".cyan().bold());
    if settings.profiles.is_empty() {
        println!("  {}", "No profiles configured".dimmed());
    } else {
        for profile in &settings.profiles {
            let active = if Some(&profile.name) == settings.active_profile.as_ref() {
                " (active)".green()
            } else {
                "".normal()
            };
            println!(
                "  {} ({}){}",
                profile.name.cyan(),
                format!("{:?}", profile.provider).to_lowercase(),
                active
            );
        }
    }
    println!();

    // Machines
    println!("{}", "Registered Machines:".cyan().bold());
    if settings.machines.is_empty() {
        println!("  {}", "No machines registered".dimmed());
    } else {
        for machine in &settings.machines {
            println!(
                "  {} ({}) - last seen: {}",
                machine.name.cyan(),
                machine.id,
                machine.last_seen.format("%Y-%m-%d %H:%M:%S").to_string().dimmed()
            );
        }
    }
    println!();

    // Config files location
    println!("{}", "Config Files:".cyan().bold());
    println!(
        "  Settings: {}",
        SettingsManager::settings_path()?.display().to_string().dimmed()
    );
    println!(
        "  PID file: {}",
        SettingsManager::pid_path()?.display().to_string().dimmed()
    );
    println!(
        "  Log file: {}",
        SettingsManager::log_path()?.display().to_string().dimmed()
    );

    Ok(())
}

/// Reset configuration to defaults
pub async fn reset() -> Result<()> {
    use dialoguer::Confirm;

    let confirm = Confirm::new()
        .with_prompt(
            "Are you sure you want to reset all configuration? This will log you out and delete all profiles.",
        )
        .default(false)
        .interact()?;

    if !confirm {
        println!("{}", "Reset cancelled.".yellow());
        return Ok(());
    }

    // Create default settings
    let default_settings = happy_core::Settings::default();
    SettingsManager::save(&default_settings).context("Failed to save default settings")?;

    println!("{} Configuration reset to defaults.", "✓".green());
    println!("{}", "  You will need to login again.".dimmed());

    Ok(())
}

/// Get the configured daemon port (default: 16790)
pub async fn get_daemon_port() -> u16 {
    if let Ok(happy_home) = SettingsManager::happy_home() {
        let port_file = happy_home.join("daemon.port");

        if let Ok(content) = tokio::fs::read_to_string(&port_file).await {
            if let Ok(port) = content.trim().parse::<u16>() {
                return port;
            }
        }
    }

    // Default port (uncommon port to avoid conflicts)
    16790
}
