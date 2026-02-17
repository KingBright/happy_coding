//! Run command - Start Claude Code with remote capabilities

use crate::config::SettingsManager;
use crate::daemon::{DaemonClient, DaemonManager};
use anyhow::{Context, Result};
use colored::Colorize;
use std::time::Duration;
use tracing::{info, warn};

pub struct RunOptions {
    pub agent: String,
    pub local: bool,
    pub tag: Option<String>,
    pub profile: Option<String>,
    pub args: Vec<String>,
}

pub async fn execute(options: RunOptions) -> Result<()> {
    info!(
        "Running agent: {}, local: {}, tag: {:?}",
        options.agent, options.local, options.tag
    );

    match options.agent.as_str() {
        "claude" => run_claude(options).await,
        "codex" => run_codex(options).await,
        _ => {
            anyhow::bail!("Unknown agent: {}. Supported: claude, codex", options.agent);
        }
    }
}

async fn run_claude(options: RunOptions) -> Result<()> {
    let settings = SettingsManager::load().context("Failed to load settings")?;

    // Check if user is authenticated
    if settings.access_token.is_none() && !options.local {
        println!("{}", "⚠️  Not authenticated. Run 'happy auth login' first or use --local".yellow());
        anyhow::bail!("Authentication required for remote mode");
    }

    // Ensure daemon is running
    let daemon_manager = DaemonManager::new();
    if !daemon_manager.is_running().await {
        println!("{}", "🔹 Starting daemon...".blue());
        daemon_manager.start().await.context("Failed to start daemon")?;

        // Wait for daemon to be ready
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Connect to daemon
    let daemon_client = DaemonClient::connect().await.context("Failed to connect to daemon")?;

    // Generate session tag if not provided
    let tag = options.tag.unwrap_or_else(generate_tag);

    println!("{}", format!("🔹 Starting Claude Code session: {}", tag).blue());

    // Start session via daemon
    let session = daemon_client
        .start_session(&tag, options.profile.as_deref())
        .await
        .context("Failed to start session")?;

    let webapp_url = format!("{}#{}", settings.webapp_url, tag);

    println!("{}", "✅ Session started!".green().bold());
    println!();
    println!("{}", "🔗 Remote URL:".cyan());
    println!("   {}", webapp_url.underline());
    println!();
    println!("{}", "📱 Scan the QR code or open the URL on your phone".dimmed());
    println!();

    // Open browser if not in local mode
    if !options.local {
        if let Err(e) = webbrowser::open(&webapp_url) {
            warn!("Failed to open browser: {}", e);
        }
    }

    // If local mode, attach to the session directly
    if options.local {
        println!("{}", "💻 Local mode - attaching to session...".blue());
        daemon_client.attach_session(&session.id).await?;
    } else {
        // Wait for the session to complete or user to interrupt
        println!("{}", "⏳ Waiting for session to complete...".dimmed());
        println!("{}", "   Press Ctrl+C to stop".dimmed());

        // Keep the CLI running until interrupted
        tokio::signal::ctrl_c().await?;

        println!();
        println!("{}", "🛑 Stopping session...".yellow());
        daemon_client.stop_session(&session.id).await?;
    }

    Ok(())
}

async fn run_codex(_options: RunOptions) -> Result<()> {
    println!("{}", "🔹 Codex support coming soon!".yellow());
    Ok(())
}

fn generate_tag() -> String {
    use rand::Rng;

    const ADJECTIVES: &[&str] = &[
        "happy", "sunny", "bright", "calm", "cool", "swift", "brave", "wise",
        "kind", "bold", "quick", "smart", "sharp", "fresh", "sweet", "warm",
        "purple", "golden", "silver", "crimson", "azure", "vivid", "gentle",
    ];

    const NOUNS: &[&str] = &[
        "dog", "cat", "bird", "fish", "wolf", "bear", "lion", "tiger",
        "eagle", "hawk", "owl", "fox", "deer", "rabbit", "turtle", "whale",
        "dolphin", "panda", "koala", "sloth", "otter", "seal", "swan", "crane",
    ];

    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    let num: u16 = rng.gen_range(1..100);

    format!("{}-{}-{}", adj, noun, num)
}
