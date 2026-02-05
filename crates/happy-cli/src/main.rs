//! Happy Coding CLI
//!
//! Universal toolkit for AI coding environments.

mod commands;

use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Parser)]
#[command(name = "happy")]
#[command(author, version, about = "Happy Coding - Universal toolkit for AI coding environments", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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

    /// Manage configuration sync
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Run a specific agent
    Run {
        /// Agent name (claude, codex, antigravity)
        agent: String,

        /// Additional arguments for the agent
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
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
enum ConfigAction {
    /// Push local config to ~/.claude/
    Push,
    /// Pull config from ~/.claude/ to local
    Pull,
    /// Show diff between local and system config
    Diff,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { name, yes } => commands::init::run(&name, yes).await,
        Commands::Build {
            target,
            watch,
            clean,
        } => commands::build::run(target, watch, clean).await,
        Commands::Dev { target } => commands::dev::run(target).await,
        Commands::Install { global, target } => commands::install::run(global, target).await,
        Commands::Validate => commands::validate::run().await,
        Commands::Doctor => commands::doctor::run().await,
        Commands::Env { action } => match action {
            EnvAction::List => commands::env::list().await,
            EnvAction::Add { name } => commands::env::add(&name).await,
            EnvAction::Use { name } => commands::env::switch(&name).await,
            EnvAction::Delete { name } => commands::env::delete(&name).await,
            EnvAction::Run { name, args } => commands::env::run(name.as_deref(), args).await,
        },
        Commands::Config { action } => match action {
            ConfigAction::Push => commands::config::push().await,
            ConfigAction::Pull => commands::config::pull().await,
            ConfigAction::Diff => commands::config::diff().await,
        },
        Commands::Run { agent, args } => commands::run::execute(agent, args).await,
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}
