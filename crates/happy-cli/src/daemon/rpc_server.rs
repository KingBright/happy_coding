use crate::daemon::rpc::{DaemonRequest, DaemonResponse};
use crate::daemon::session_manager::DaemonSessionManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

pub struct RpcServer {
    session_manager: DaemonSessionManager,
    port: u16,
}

impl RpcServer {
    pub fn new(session_manager: DaemonSessionManager, port: u16) -> Self {
        Self {
            session_manager,
            port,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Daemon RPC server listening on {}", addr);

        loop {
            let (mut socket, _) = listener.accept().await?;
            let session_manager = self.session_manager.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(&mut socket, session_manager).await {
                    error!("RPC connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    socket: &mut tokio::net::TcpStream,
    session_manager: DaemonSessionManager,
) -> Result<()> {
    // Read length-prefixed or newline-delimited JSON?
    // Let's use simple length-prefixed for reliability or just one request per connection for simplicity.
    // "One request per connection" is simplest for CLI tools.

    let mut buf = Vec::new();
    socket.read_to_end(&mut buf).await?;

    if buf.is_empty() {
        return Ok(());
    }

    let request: DaemonRequest = serde_json::from_slice(&buf)?;
    debug!("Received RPC request: {:?}", request);

    let response = match request {
        DaemonRequest::StartSession {
            id,
            tag,
            token,
            server_url,
            cwd,
        } => match session_manager
            .start_session(id, tag, token, server_url, cwd)
            .await
        {
            Ok(session_id) => DaemonResponse::SessionStarted { session_id },
            Err(e) => DaemonResponse::Error(e.to_string()),
        },
        DaemonRequest::StopSession { session_id } => {
            match session_manager.stop_session(&session_id).await {
                Ok(_) => DaemonResponse::Ok,
                Err(e) => DaemonResponse::Error(e.to_string()),
            }
        }
        DaemonRequest::ListSessions => {
            let sessions = session_manager.list_sessions().await;
            DaemonResponse::Sessions(sessions)
        }
        DaemonRequest::Shutdown => {
            // How to handle shutdown? Maybe send response then exit?
            // For now, let's just return Ok and maybe handling shutdown in the main loop is better.
            DaemonResponse::Ok
        }
    };

    let response_bytes = serde_json::to_vec(&response)?;
    socket.write_all(&response_bytes).await?;

    Ok(())
}
