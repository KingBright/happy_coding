//! Run command - Start Claude Code with remote capabilities

use crate::commands::auth;
use crate::config::SettingsManager;
use crate::daemon::{DaemonClient, DaemonManager};
use anyhow::{Context, Result};
use colored::Colorize;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn};

pub struct RunOptions {
    pub agent: String,
    /// Enable remote sync mode (default: false = local-only)
    pub remote: bool,
    pub tag: Option<String>,
    #[allow(dead_code)]
    pub profile: Option<String>,
    #[allow(dead_code)]
    pub args: Vec<String>,
}

pub async fn execute(options: RunOptions) -> Result<()> {
    info!(
        "Running agent: {}, remote: {}, tag: {:?}",
        options.agent, options.remote, options.tag
    );

    match options.agent.as_str() {
        "claude" => run_claude(options).await,
        "codex" => run_codex(options).await,
        _ => {
            anyhow::bail!("Unknown agent: {}. Supported: claude, codex", options.agent);
        }
    }
}

/// Ensure user is authenticated, auto-login from config or prompt if needed
async fn ensure_authenticated() -> Result<()> {
    let settings = SettingsManager::load().context("Failed to load settings")?;

    // First, check if we have a valid token
    if settings.access_token.is_some() {
        let client = crate::api::Client::new();
        if let Some(ref token) = settings.access_token {
            match client.get_user_info(token).await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    println!(
                        "{}",
                        "⚠️  Token expired, trying to re-authenticate...".yellow()
                    );
                    // Token expired, try auto-login from config
                }
            }
        }
    }

    // Try auto-login from config credentials
    if settings.email.is_some() && settings.password.is_some() {
        let email = settings.email.unwrap();
        let password = settings.password.unwrap();

        println!(
            "{}",
            "🔹 Auto-login with saved credentials...".blue().dimmed()
        );

        let client = crate::api::Client::new();
        match client.login(&email, &password).await {
            Ok(tokens) => {
                // Save tokens
                let mut new_settings = SettingsManager::load()?;
                new_settings.access_token = Some(tokens.access_token.clone());
                new_settings.refresh_token = Some(tokens.refresh_token);
                new_settings.email = Some(email);
                new_settings.password = Some(password);
                SettingsManager::save(&new_settings)?;

                // Get user info
                if let Ok(user) = client.get_user_info(&tokens.access_token).await {
                    new_settings.user_id = Some(user.id);
                    SettingsManager::save(&new_settings)?;
                }

                println!("{}", "✅ Auto-login successful!".green());
                return Ok(());
            }
            Err(e) => {
                println!("{}", format!("⚠️  Auto-login failed: {}", e).yellow());
                // Fall through to interactive login
            }
        }
    }

    // No valid token and no config credentials, prompt for login
    println!("{}", "🔹 Welcome to Happy Remote!".blue().bold());
    println!();
    println!("This appears to be your first time using remote mode.");
    println!("You need to login or create an account to continue.");
    println!();

    let choices = vec!["Login with existing account", "Create new account", "Exit"];
    let selection = dialoguer::Select::new()
        .with_prompt("What would you like to do?")
        .items(&choices)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // Login
            auth::login_interactive().await?;
            Ok(())
        }
        1 => {
            // Register then login
            register_interactive().await?;
            Ok(())
        }
        _ => {
            anyhow::bail!("Authentication required for remote mode");
        }
    }
}

/// Interactive registration flow
async fn register_interactive() -> Result<()> {
    use crate::api::Client;

    println!("{}", "🔹 Create Account".blue().bold());
    println!();

    let email: String = dialoguer::Input::new()
        .with_prompt("Email")
        .interact_text()?;

    // Basic email validation
    if !email.contains('@') || !email.contains('.') {
        anyhow::bail!("Please enter a valid email address");
    }

    let name: String = dialoguer::Input::new()
        .with_prompt("Name (optional)")
        .allow_empty(true)
        .interact_text()?;

    let password: String = dialoguer::Password::new()
        .with_prompt("Password")
        .interact()?;

    // Basic password validation
    if password.len() < 6 {
        anyhow::bail!("Password must be at least 6 characters");
    }

    let confirm_password: String = dialoguer::Password::new()
        .with_prompt("Confirm password")
        .interact()?;

    if password != confirm_password {
        anyhow::bail!("Passwords do not match");
    }

    println!();
    println!("{}", "📝 Creating account...".dimmed());

    let client = Client::new();
    let tokens = client
        .register(
            &email,
            &password,
            if name.is_empty() { None } else { Some(&name) },
        )
        .await
        .context("Registration failed")?;

    // Save tokens and credentials
    let mut settings = SettingsManager::load()?;
    settings.access_token = Some(tokens.access_token.clone());
    settings.refresh_token = Some(tokens.refresh_token);
    settings.email = Some(email.clone());
    settings.password = Some(password.clone());
    SettingsManager::save(&settings)?;

    // Get user info
    match client.get_user_info(&tokens.access_token).await {
        Ok(user) => {
            settings.user_id = Some(user.id.clone());
            SettingsManager::save(&settings)?;

            println!();
            println!("{}", "✅ Account created successfully!".green().bold());
            println!();
            println!(
                "   Welcome, {}!",
                user.name.as_deref().unwrap_or(&user.email).cyan()
            );
        }
        Err(_) => {
            println!();
            println!("{}", "✅ Account created successfully!".green().bold());
        }
    }

    Ok(())
}

async fn run_claude(options: RunOptions) -> Result<()> {
    // Generate session tag
    let tag = options.tag.clone().unwrap_or_else(generate_tag);

    if options.remote {
        // Remote mode: authenticate, start daemon, sync to cloud
        run_claude_remote(&tag, options).await
    } else {
        // Local mode: just run Claude in PTY directly
        run_claude_local(&tag).await
    }
}

/// Local mode: Spawn Claude in PTY and interact directly in terminal
async fn run_claude_local(_tag: &str) -> Result<()> {
    println!("{}", "🔹 Starting Claude Code...".blue());
    println!();

    // Spawn PTY with claude process
    run_local_pty(_tag).await
}

/// Remote mode: Run with cloud sync
async fn run_claude_remote(tag: &str, options: RunOptions) -> Result<()> {
    // Ensure user is authenticated
    ensure_authenticated().await?;

    let settings = SettingsManager::load().context("Failed to load settings")?;

    // Ensure daemon is running
    let daemon_manager = DaemonManager::new();
    if !daemon_manager.is_running().await {
        println!("{}", "🔹 Starting daemon...".blue());
        daemon_manager
            .start()
            .await
            .context("Failed to start daemon")?;

        // Wait for daemon to be ready
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Connect to daemon
    let daemon_client = DaemonClient::connect()
        .await
        .context("Failed to connect to daemon")?;

    println!(
        "{}",
        format!("🔹 Starting Claude Code session: {}", tag).blue()
    );

    // Get machine name dynamically (use macOS ComputerName if available)
    let machine_name = get_machine_name();

    // Get shell's current working directory (PWD env var is more reliable than current_dir)
    let cwd = std::env::var("PWD")
        .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|_| "/".to_string());

    // Register with cloud
    println!("{}", "🔹 Registering session with cloud...".blue().dimmed());
    let api_client = crate::api::Client::new();
    let cloud_id = match api_client
        .create_session(
            settings.access_token.as_deref().unwrap_or_default(),
            tag,
            options.profile.as_deref(),
            &settings.machine_id,
            &machine_name,
            &cwd,
        )
        .await
    {
        Ok(info) => Some(info.id),
        Err(e) => {
            warn!("Failed to register session with cloud: {}", e);
            None
        }
    };

    // Get shell's current working directory (PWD env var is more reliable than current_dir)
    let cwd = std::env::var("PWD")
        .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|_| "/".to_string());

    // Start session via daemon
    let session = daemon_client
        .start_session(cloud_id, tag, &cwd)
        .await
        .context("Failed to start session")?;

    let webapp_url = format!("{}#{}", settings.webapp_url, tag);

    println!("{}", "✅ Session started!".green().bold());
    println!();
    println!("{}", "🔗 Remote URL:".cyan());
    println!("   {}", webapp_url.underline());
    println!();
    println!(
        "{}",
        "📱 Scan the QR code or open the URL on your phone".dimmed()
    );
    println!();

    // Open browser
    if let Err(e) = webbrowser::open(&webapp_url) {
        warn!("Failed to open browser: {}", e);
    }

    println!("{}", "💻 Attaching to session...".blue());
    daemon_client.attach_session(&session.id).await?;

    Ok(())
}

/// Run a local PTY session with Claude
async fn run_local_pty(tag: &str) -> Result<()> {
    use nix::sys::termios::{self, SetArg};
    use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
    use std::io::{Read, Write};
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

    // Get terminal size
    let (cols, rows) = get_terminal_size()?;

    // Create PTY
    let pty_system = NativePtySystem::default();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // Build command
    let mut cmd = CommandBuilder::new("claude");
    cmd.cwd(std::env::current_dir().unwrap_or_default());
    cmd.env("HAPPY_SESSION_TAG", tag);
    cmd.env("TERM", "xterm-256color");

    // Spawn the process
    let mut child = pair.slave.spawn_command(cmd)?;

    // Get PTY reader/writer
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    // Set terminal to raw mode
    let stdin_handle = std::io::stdin();
    let stdin_fd = stdin_handle.as_fd();
    let original_termios = termios::tcgetattr(stdin_fd)?;
    let mut raw_termios = original_termios.clone();
    termios::cfmakeraw(&mut raw_termios);
    termios::tcsetattr(stdin_fd, SetArg::TCSANOW, &raw_termios)?;

    // Guard to restore terminal settings on drop
    struct TermiosGuard {
        fd: i32,
        original: termios::Termios,
    }
    impl Drop for TermiosGuard {
        fn drop(&mut self) {
            unsafe {
                let fd = BorrowedFd::borrow_raw(self.fd);
                let _ = termios::tcsetattr(fd, SetArg::TCSANOW, &self.original);
            }
        }
    }
    let _guard = TermiosGuard {
        fd: stdin_handle.as_raw_fd(),
        original: original_termios,
    };

    // Use spawn_blocking for PTY reader to avoid runtime issues
    let (output_tx, mut output_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1000);

    let reader_handle = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    // Use blocking_send which doesn't require async context
                    use tokio::sync::mpsc::error::TrySendError;
                    match output_tx.try_send(buf[..n].to_vec()) {
                        Ok(_) => {}
                        Err(TrySendError::Full(_)) => {
                            // Channel full, skip this output
                        }
                        Err(TrySendError::Closed(_)) => break,
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Main loop: forward I/O between terminal and PTY
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut input_buf = [0u8; 4096];

    loop {
        tokio::select! {
            // Read from stdin, write to PTY
            result = stdin.read(&mut input_buf) => {
                let n = result?;
                if n == 0 { break; }
                writer.write_all(&input_buf[..n])?;
                writer.flush()?;
            }

            // Read from PTY, write to stdout
            Some(data) = output_rx.recv() => {
                stdout.write_all(&data).await?;
                stdout.flush().await?;
            }

            else => break,
        }

        // Check if child process is still running
        if let Some(exit_status) = child.try_wait()? {
            tracing::info!("Child process exited with status: {:?}", exit_status);
            break;
        }
    }

    // Wait for child to fully exit
    let _ = child.wait();
    reader_handle.abort();

    Ok(())
}

/// Get current terminal size
fn get_terminal_size() -> Result<(u16, u16)> {
    use std::io::IsTerminal;

    if !std::io::stdout().is_terminal() {
        return Ok((80, 24)); // Default size
    }

    unsafe {
        let mut winsize: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) == 0 {
            Ok((winsize.ws_col, winsize.ws_row))
        } else {
            Ok((80, 24))
        }
    }
}

async fn run_codex(_options: RunOptions) -> Result<()> {
    println!("{}", "🔹 Codex support coming soon!".yellow());
    Ok(())
}

fn generate_tag() -> String {
    use rand::Rng;

    const ADJECTIVES: &[&str] = &[
        "happy", "sunny", "bright", "calm", "cool", "swift", "brave", "wise", "kind", "bold",
        "quick", "smart", "sharp", "fresh", "sweet", "warm", "purple", "golden", "silver",
        "crimson", "azure", "vivid", "gentle",
    ];

    const NOUNS: &[&str] = &[
        "dog", "cat", "bird", "fish", "wolf", "bear", "lion", "tiger", "eagle", "hawk", "owl",
        "fox", "deer", "rabbit", "turtle", "whale", "dolphin", "panda", "koala", "sloth", "otter",
        "seal", "swan", "crane",
    ];

    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    let num: u16 = rng.gen_range(1..100);

    format!("{}-{}-{}", adj, noun, num)
}

/// Get machine name - prefer macOS ComputerName for user-friendly name
fn get_machine_name() -> String {
    happy_core::utils::get_machine_name()
}
