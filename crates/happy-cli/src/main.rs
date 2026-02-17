//! Happy Coding CLI
//!
//! Universal toolkit for AI coding environments with remote control support.

mod api;
mod commands;
mod config;
mod daemon;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "happy")]
#[command(author, version, about = "Happy Coding - Universal toolkit for AI coding environments", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new project
    Init {
        /// Project name
        #[arg(default_value = ".")]
        name: String,

        /// Skip interactive prompts
        #[arg(short, long)]
        yes: bool,
    },

    /// Build for all configured platforms
    Build {
        /// Target specific platform
        #[arg(short, long)]
        target: Option<String>,

        /// Watch for changes and rebuild
        #[arg(short, long)]
        watch: bool,

        /// Clean output directories before build
        #[arg(long)]
        clean: bool,
    },

    /// Start development mode (watch + build)
    Dev {
        /// Target specific platform
        #[arg(short, long)]
        target: Option<String>,
    },

    /// Install built artifacts
    Install {
        /// Install globally
        #[arg(short, long)]
        global: bool,

        /// Target platform
        #[arg(short, long)]
        target: Option<String>,
    },

    /// Validate configuration
    Validate,

    /// Diagnose environment and dependencies
    Doctor,

    /// Manage AI agent environments (Claude/Codex)
    Env {
        #[command(subcommand)]
        action: EnvAction,
    },

    /// Manage configuration sync (local)
    #[command(name = "local-config")]
    LocalConfig {
        #[command(subcommand)]
        action: LocalConfigAction,
    },

    /// Run a specific agent
    Run {
        /// Agent name (claude, codex)
        #[arg(default_value = "claude")]
        agent: String,

        /// Enable remote sync mode (syncs to cloud for web/mobile access)
        #[arg(long)]
        remote: bool,

        /// Session tag
        #[arg(short, long)]
        tag: Option<String>,

        /// AI profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// Additional arguments for the agent
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Manage the background daemon (remote mode)
    #[command(name = "daemon")]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Authentication management (remote mode)
    #[command(name = "auth")]
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Connect to AI vendors
    #[command(name = "connect")]
    Connect {
        /// Vendor to connect (anthropic, openai, azure)
        vendor: String,
    },

    /// Manage AI profiles (remote mode)
    #[command(name = "profile")]
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Push notifications (remote mode)
    #[command(name = "notify")]
    Notify { message: String },

    /// Configuration management (remote mode)
    #[command(name = "config")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum EnvAction {
    /// List all environments
    List,
    /// Add a new environment
    Add {
        /// Environment name
        name: String,
    },
    /// Switch to an environment
    Use {
        /// Environment name
        name: String,
    },
    /// Delete an environment
    Delete {
        /// Environment name
        name: String,
    },
    /// Run Claude with a specific environment
    Run {
        /// Environment name (optional, uses default if not specified)
        name: Option<String>,
        /// Additional arguments to pass to claude
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum LocalConfigAction {
    /// Push local config to ~/.claude/
    Push,
    /// Pull config from ~/.claude/ to local
    Pull,
    /// Show diff between local and system config
    Diff,
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Restart the daemon
    Restart,
    /// Check daemon status
    Status,
    /// View daemon logs
    Logs {
        /// Follow logs
        #[arg(short, long)]
        follow: bool,
    },
    /// Internal command to run the daemon process
    #[clap(hide = true)]
    Run,
}

#[derive(Subcommand)]
enum AuthAction {
    /// Login to Happy Remote
    Login {
        /// Email address (optional - will prompt if not provided)
        #[arg(short, long)]
        email: Option<String>,
        /// Password (optional - will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Logout
    Logout,
    /// Show current user
    Whoami,
    /// List access keys
    Keys,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List profiles
    List,
    /// Add a new profile
    Add { name: String },
    /// Set active profile
    Use { name: String },
    /// Delete a profile
    Remove { name: String },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set the remote server URL
    SetServer {
        /// Server URL (e.g., https://happy.example.com)
        url: String,
    },
    /// Set the daemon WebSocket port
    SetDaemonPort {
        /// Port number (default: 16790)
        port: u16,
    },
    /// Show current configuration
    Show,
    /// Reset to default configuration
    Reset,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging (except for daemon run which has its own file logger)
    let is_daemon_run = matches!(
        cli.command,
        Commands::Daemon {
            action: DaemonAction::Run
        }
    );

    if !is_daemon_run {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(if cli.verbose {
                "happy_cli=debug,happy_core=debug"
            } else {
                "happy_cli=info"
            })
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .finish();

        tracing::subscriber::set_global_default(subscriber)?;
        info!("Starting Happy Coding CLI");
    }

    let result = match cli.command {
        // Local development commands
        Commands::Init { name, yes } => commands::init::run(&name, yes).await,
        Commands::Build {
            target,
            watch,
            clean,
        } => commands::build::run(target, watch, clean).await,
        Commands::Dev { target } => commands::dev::run(target).await,
        Commands::Install { global, target } => commands::install::run(global, target).await,
        Commands::Validate => commands::validate::run().await,
        Commands::Doctor => {
            // Try remote doctor first, or fallback?
            // Remote doctor::execute() seems generic.
            commands::doctor::execute().await
        }
        Commands::Env { action } => match action {
            EnvAction::List => commands::env::list().await,
            EnvAction::Add { name } => commands::env::add(&name).await,
            EnvAction::Use { name } => commands::env::switch(&name).await,
            EnvAction::Delete { name } => commands::env::delete(&name).await,
            EnvAction::Run { name, args } => commands::env::run(name.as_deref(), args).await,
        },
        Commands::LocalConfig { action } => match action {
            LocalConfigAction::Push => commands::local_config::push().await,
            LocalConfigAction::Pull => commands::local_config::pull().await,
            LocalConfigAction::Diff => commands::local_config::diff().await,
        },

        // Unified Run command
        Commands::Run {
            agent,
            remote,
            tag,
            profile,
            args,
        } => {
            commands::run::execute(commands::run::RunOptions {
                agent,
                remote,
                tag,
                profile,
                args,
            })
            .await
        }

        // Remote commands
        Commands::Daemon { action } => match action {
            DaemonAction::Start => commands::daemon::start().await,
            DaemonAction::Stop => commands::daemon::stop().await,
            DaemonAction::Restart => commands::daemon::restart().await,
            DaemonAction::Status => commands::daemon::status().await,
            DaemonAction::Logs { follow } => commands::daemon::logs(follow).await,
            DaemonAction::Run => commands::daemon::run().await,
        },
        Commands::Auth { action } => match action {
            AuthAction::Login { email, password } => match (email, password) {
                (Some(e), Some(p)) => commands::auth::login_non_interactive(&e, &p).await,
                _ => commands::auth::login_interactive().await,
            },
            AuthAction::Logout => commands::auth::logout().await,
            AuthAction::Whoami => commands::auth::whoami().await,
            AuthAction::Keys => commands::auth::keys().await,
        },
        Commands::Connect { vendor } => commands::connect::execute(&vendor).await,
        Commands::Profile { action } => match action {
            ProfileAction::List => commands::profile::list().await,
            ProfileAction::Add { name } => commands::profile::add(&name).await,
            ProfileAction::Use { name } => commands::profile::use_profile(&name).await,
            ProfileAction::Remove { name } => commands::profile::remove(&name).await,
        },
        Commands::Notify { message } => commands::notify::execute(&message).await,
        Commands::Config { action } => match action {
            ConfigAction::SetServer { url } => commands::config::set_server(&url).await,
            ConfigAction::SetDaemonPort { port } => commands::config::set_daemon_port(port).await,
            ConfigAction::Show => commands::config::show().await,
            ConfigAction::Reset => commands::config::reset().await,
        },
    };

    if let Err(ref e) = result {
        error!("Command failed: {}", e);
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }

    result
}
