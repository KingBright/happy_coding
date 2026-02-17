//! Daemon management commands

use anyhow::{Context, Result};
use colored::Colorize;

pub async fn start() -> Result<()> {
    println!("{}", "🔹 Starting Happy Remote daemon...".blue());

    let daemon_manager = crate::daemon::DaemonManager::new();

    if daemon_manager.is_running().await {
        println!("{}", "✅ Daemon is already running".green());
        return Ok(());
    }

    daemon_manager
        .start()
        .await
        .context("Failed to start daemon")?;

    println!("{}", "✅ Daemon started successfully".green());
    Ok(())
}

pub async fn stop() -> Result<()> {
    println!("{}", "🔹 Stopping Happy Remote daemon...".blue());

    let daemon_manager = crate::daemon::DaemonManager::new();

    if !daemon_manager.is_running().await {
        println!("{}", "⚠️  Daemon is not running".yellow());
        return Ok(());
    }

    daemon_manager
        .stop()
        .await
        .context("Failed to stop daemon")?;

    println!("{}", "✅ Daemon stopped successfully".green());
    Ok(())
}

pub async fn restart() -> Result<()> {
    println!("{}", "🔹 Restarting Happy Remote daemon...".blue());

    let daemon_manager = crate::daemon::DaemonManager::new();

    if daemon_manager.is_running().await {
        daemon_manager.stop().await?;
    }

    daemon_manager
        .start()
        .await
        .context("Failed to restart daemon")?;

    println!("{}", "✅ Daemon restarted successfully".green());
    Ok(())
}

pub async fn status() -> Result<()> {
    let daemon_manager = crate::daemon::DaemonManager::new();

    if daemon_manager.is_running().await {
        println!("{}", "✅ Daemon is running".green());

        // Try to get more info from daemon
        match crate::daemon::DaemonClient::connect().await {
            Ok(client) => {
                if let Ok(info) = client.get_info().await {
                    println!("   Version: {}", info.version);
                    println!("   Uptime: {}s", info.uptime_secs);
                    println!("   Active sessions: {}", info.active_sessions);
                }
            }
            Err(_) => {
                println!("   (Unable to communicate with daemon)");
            }
        }
    } else {
        println!("{}", "⚠️  Daemon is not running".yellow());
    }

    Ok(())
}

pub async fn logs(follow: bool) -> Result<()> {
    let log_path = crate::config::SettingsManager::log_path()?;

    if !log_path.exists() {
        println!("{}", "⚠️  No log file found".yellow());
        return Ok(());
    }

    if follow {
        println!("{}", "🔹 Following daemon logs (Ctrl+C to exit)...".blue());

        // Use tokio::process::Command to run tail -f
        let mut cmd = tokio::process::Command::new("tail")
            .arg("-f")
            .arg(&log_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        cmd.wait().await?;
    } else {
        let content = tokio::fs::read_to_string(&log_path).await?;
        println!("{}", content);
    }

    Ok(())
}

pub async fn run() -> Result<()> {
    use crate::daemon::rpc_server::RpcServer;
    use crate::daemon::session_manager::DaemonSessionManager;
    use tracing::{error, info};

    println!("DEBUG: Starting daemon run command...");

    // 1. Initialize file logging
    let log_path = crate::config::SettingsManager::log_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("daemon.log"));

    // Ensure parent dir exists
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file_appender = tracing_appender::rolling::never(
        log_path.parent().unwrap_or(std::path::Path::new(".")),
        log_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("daemon.log")),
    );

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false) // cleaner logs
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Initializing Daemon...");
    println!("DEBUG: Initializing Daemon...");

    // 2. Initialize Session Manager (needs state_dir)
    let happy_home = crate::config::SettingsManager::happy_home()?;
    println!("DEBUG: Happy Home: {:?}", happy_home);
    let session_manager = DaemonSessionManager::new(happy_home.clone())
        .context("Failed to initialize session manager")?;
    println!("DEBUG: Session Manager initialized");

    // 3. Start RPC Server
    let rpc_port = 16792;
    let rpc_server = RpcServer::new(session_manager.clone(), rpc_port);
    println!("DEBUG: RPC Server created");

    // 4. Config for WebSocket Server
    let ws_port = 16790;
    let ws_addr = std::net::SocketAddr::from(([127, 0, 0, 1], ws_port));

    info!("Starting RPC Server on {}", rpc_port);
    info!("Starting WebSocket Server on {}", ws_port);
    println!("DEBUG: Configured ports");

    // 5. Run servers
    println!("DEBUG: Starting servers...");

    // Spawn session recovery in background
    let session_manager_clone = session_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = session_manager_clone.recover_sessions().await {
            error!("Failed to recover sessions: {}", e);
        }
    });

    tokio::select! {
        res = rpc_server.run() => {
            if let Err(e) = res {
                error!("RPC Server failed: {}", e);
                return Err(e);
            }
        }
        res = crate::daemon::server::start_daemon_server(session_manager.multiplexer(), ws_addr) => {
            if let Err(e) = res {
                error!("WebSocket Server failed: {}", e);
                return Err(e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Shutting down daemon...");
        }
    }

    Ok(())
}
