//! Notify command - Push notifications

use crate::api::Client;
use crate::config::SettingsManager;
use anyhow::Result;
use colored::Colorize;

pub async fn execute(message: &str) -> Result<()> {
    let settings = SettingsManager::load()?;

    if settings.access_token.is_none() {
        println!("{}", "⚠️  Not logged in".yellow());
        return Ok(());
    }

    let client = Client::new();
    let token = settings.access_token.unwrap();

    match client.send_notification(&token, message).await {
        Ok(_) => {
            println!("{}", "✅ Notification sent".green());
        }
        Err(e) => {
            println!("{}", format!("⚠️  Failed to send notification: {}", e).yellow());
        }
    }

    Ok(())
}
