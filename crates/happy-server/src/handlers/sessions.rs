//! Session handlers

use crate::AppState;
use axum::{
    extract::{Path, State},
    http::header::HeaderMap,
    http::StatusCode,
    Json,
};
use happy_core::Session;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    sessions: Vec<Session>,
}

fn extract_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
}

fn extract_machine_id(headers: &HeaderMap) -> String {
    headers
        .get("X-Machine-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn extract_machine_name(headers: &HeaderMap) -> String {
    headers
        .get("X-Machine-Name")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown Machine".to_string())
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionListResponse>, StatusCode> {
    let token = extract_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token
    let user_id = match state.auth_service.validate_token(token).await {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    match state.session_manager.list_user_sessions(&user_id).await {
        Ok(sessions) => Ok(Json(SessionListResponse { sessions })),
        Err(e) => {
            tracing::error!("Failed to list sessions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    tag: String,
    profile: Option<String>,
    cwd: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    session: Session,
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req_body): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let token = extract_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token
    let user_id = match state.auth_service.validate_token(token).await {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // Extract machine info from headers
    let machine_id = extract_machine_id(&headers);
    let machine_name = extract_machine_name(&headers);

    // Register/update machine info
    if let Err(e) = state
        .machine_registry
        .register_machine(
            &user_id,
            &machine_id,
            &machine_name,
            happy_core::Platform::current(),
        )
        .await
    {
        tracing::warn!("Failed to register machine: {}", e);
    }

    // Get cwd from request, default to "/" if not provided
    let cwd = req_body.cwd.unwrap_or_else(|| "/".to_string());

    match state
        .session_manager
        .create_session(&user_id, &machine_id, &machine_name, &req_body.tag, &cwd)
        .await
    {
        Ok(session) => Ok(Json(SessionResponse { session })),
        Err(e) => {
            tracing::error!("Failed to create session: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, StatusCode> {
    let token = extract_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token
    let user_id = match state.auth_service.validate_token(token).await {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    match state.session_manager.get_session(&id).await {
        Ok(Some(session)) => {
            // Verify user owns this session
            if session.user_id != user_id {
                return Err(StatusCode::FORBIDDEN);
            }
            Ok(Json(SessionResponse { session }))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get session: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let token = extract_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token
    let user_id = match state.auth_service.validate_token(token).await {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // First get session to verify ownership and check status
    let session = match state.session_manager.get_session(&id).await {
        Ok(Some(session)) => {
            if session.user_id != user_id {
                return Err(StatusCode::FORBIDDEN);
            }
            session
        }
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get session: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // If session is running, terminate it first. Otherwise, permanently remove it.
    use happy_core::SessionStatus;
    use happy_types::ServerMessage;

    match session.status {
        SessionStatus::Running | SessionStatus::Paused => {
            // Soft delete - mark as terminated
            match state.session_manager.terminate_session(&id).await {
                Ok(_) => {
                    // Notify CLI bridge to stop the session
                    state
                        .conn_manager
                        .forward_to_cli(
                            &id,
                            ServerMessage::SessionStopped {
                                session_id: id.clone(),
                            },
                        )
                        .await;
                    Ok(StatusCode::NO_CONTENT)
                }
                Err(e) => {
                    tracing::error!("Failed to terminate session: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        SessionStatus::Initializing | SessionStatus::Terminated => {
            // Hard delete - permanently remove zombie/completed sessions
            match state.session_manager.remove_session(&id).await {
                Ok(_) => {
                    // Notify CLI bridge to delete the session
                    state
                        .conn_manager
                        .forward_to_cli(
                            &id,
                            ServerMessage::SessionDeleted {
                                session_id: id.clone(),
                            },
                        )
                        .await;
                    Ok(StatusCode::NO_CONTENT)
                }
                Err(e) => {
                    tracing::error!("Failed to remove session: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    }
}
