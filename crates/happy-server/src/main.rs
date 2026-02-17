//! Happy Remote Server
//!
//! The cloud server component for Happy Remote - handles WebSocket connections,
//! session management, and real-time synchronization.
//!
//! Uses SQLite (embedded) instead of PostgreSQL for simplicity.

mod extractors;
mod handlers;
mod services;
mod storage;

use anyhow::{Context, Result};
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use handlers::ws::ConnectionManager;
use services::{AuthService, MachineRegistry, SessionManager};
use storage::{Database, MemoryCache};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub cache: Arc<MemoryCache>,
    pub session_manager: Arc<SessionManager>,
    pub machine_registry: Arc<MachineRegistry>,
    pub auth_service: Arc<AuthService>,
    pub conn_manager: Arc<ConnectionManager>,
}

#[tokio::main]
async fn main() {
    // Set up panic hook to log crashes
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()));
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        eprintln!("[PANIC] at {:?}: {}", location, payload);
        tracing::error!("PANIC at {:?}: {}", location, payload);
    }));

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("[FATAL] Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    info!(
        "Starting Happy Remote Server v{}",
        env!("CARGO_PKG_VERSION")
    );
    info!("PID: {}", std::process::id());

    if let Err(e) = run_server().await {
        error!("Server failed: {:#}", e);
        std::process::exit(1);
    }
}

async fn run_server() -> Result<()> {
    // Load configuration
    info!("Loading configuration...");
    let config = load_config()
        .await
        .context("Failed to load configuration")?;
    info!(
        "Config loaded: bind={}, db={}",
        config.bind_address, config.database_path
    );

    // Initialize SQLite database
    info!("Initializing SQLite database...");
    let db = Arc::new(
        Database::new(&config.database_path)
            .await
            .context("Failed to initialize database")?,
    );
    info!("SQLite database initialized at: {}", config.database_path);

    // Initialize in-memory cache (replaces Redis)
    info!("Initializing in-memory cache...");
    let cache = Arc::new(MemoryCache::new());
    info!("In-memory cache initialized");

    // Initialize services
    info!("Initializing services...");
    let session_manager = Arc::new(SessionManager::new(db.clone(), cache.clone()));
    let machine_registry = Arc::new(MachineRegistry::new(db.clone(), cache.clone()));
    let auth_service = Arc::new(AuthService::new(db.clone(), config.jwt_secret.clone()));
    info!("Services initialized");

    // Create connection manager
    let conn_manager = Arc::new(ConnectionManager::new());

    // Create app state
    let state = AppState {
        db,
        cache,
        session_manager,
        machine_registry,
        auth_service,
        conn_manager,
    };

    // Static files directory
    let static_dir =
        std::env::var("STATIC_DIR").unwrap_or_else(|_| "/opt/happy-remote/frontend".to_string());
    info!("Static files directory: {}", static_dir);

    // Build router
    info!("Building HTTP router...");

    let index_path = PathBuf::from(&static_dir).join("index.html");

    let app = Router::new()
        // Health check
        .route("/health", get(handlers::health))
        // WebSocket endpoint
        .route("/ws", get(handlers::ws::handler))
        // REST API routes
        .nest("/api/v1", api_routes())
        // Static files
        .nest_service(
            "/pkg",
            ServeDir::new(PathBuf::from(&static_dir).join("pkg")),
        )
        .nest_service(
            "/assets",
            ServeDir::new(PathBuf::from(&static_dir).join("assets")),
        )
        // SPA fallback - all routes serve index.html
        .fallback_service(ServeFile::new(index_path))
        // Layers
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr: SocketAddr = config
        .bind_address
        .parse()
        .context("Failed to parse bind address")?;
    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;

    info!("Server ready to accept connections");
    axum::serve(listener, app).await.context("Server error")?;

    Ok(())
}

fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/refresh", post(handlers::auth::refresh))
        .route("/users/me", get(handlers::users::me))
        .route(
            "/sessions",
            get(handlers::sessions::list).post(handlers::sessions::create),
        )
        .route(
            "/sessions/:id",
            get(handlers::sessions::get).delete(handlers::sessions::delete),
        )
        .route(
            "/machines",
            get(handlers::machines::list).post(handlers::machines::register),
        )
        .route("/machines/:id", get(handlers::machines::get))
}

#[derive(Debug, Clone)]
struct Config {
    bind_address: String,
    database_path: String,
    jwt_secret: String,
    data_dir: PathBuf,
}

async fn load_config() -> Result<Config> {
    info!("Loading configuration from environment...");

    // Get data directory
    let data_dir = std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/happy-remote/data"));
    info!("Data directory: {}", data_dir.display());

    // Ensure data directory exists
    if let Err(e) = tokio::fs::create_dir_all(&data_dir).await {
        return Err(anyhow::anyhow!(
            "Failed to create data directory {}: {}",
            data_dir.display(),
            e
        ));
    }

    // Verify data directory is writable
    match tokio::fs::metadata(&data_dir).await {
        Ok(meta) => {
            info!(
                "Data directory exists, permissions: {:o}",
                meta.permissions().mode()
            );
        }
        Err(e) => {
            warn!("Cannot stat data directory: {}", e);
        }
    }

    let database_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| {
        let path = data_dir.join("happy_remote.db");
        path.to_string_lossy().to_string()
    });

    let bind_address =
        std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:16789".to_string());

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        warn!("JWT_SECRET not set, using default (insecure for production)");
        "change-me-in-production".to_string()
    });

    Ok(Config {
        bind_address,
        database_path,
        jwt_secret,
        data_dir,
    })
}
