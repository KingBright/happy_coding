//! Authentication handlers

use crate::AppState;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    id: String,
    email: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    email: String,
    name: Option<String>,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    info!("Login attempt for: {}", req.email);

    // Use AuthService to login
    let tokens = state
        .auth_service
        .login(&req.email, &req.password)
        .await
        .map_err(|e| {
            error!("Login error: {}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Get user info from token
    let user_id = state
        .auth_service
        .validate_token(&tokens.access_token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!("Login successful for: {}", req.email);

    Ok(Json(LoginResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
        user: UserInfo {
            id: user_id,
            email: req.email,
            name: None, // We don't have name in login response for now
        },
    }))
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    info!("Registration attempt for: {}", req.email);

    // Validate email
    if !req.email.contains('@') {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate password length
    if req.password.len() < 6 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if user already exists
    if state
        .db
        .get_user_by_email(&req.email)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .is_some()
    {
        return Err(StatusCode::CONFLICT);
    }

    // Use AuthService to register
    let tokens = state
        .auth_service
        .register(&req.email, &req.password, req.name.as_deref())
        .await
        .map_err(|e| {
            error!("Registration error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get user id from token
    let user_id = state
        .auth_service
        .validate_token(&tokens.access_token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!("Registration successful for: {}", req.email);

    Ok(Json(LoginResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
        user: UserInfo {
            id: user_id,
            email: req.email,
            name: req.name,
        },
    }))
}

pub async fn refresh(State(_state): State<AppState>) -> Result<Json<LoginResponse>, StatusCode> {
    // TODO: Implement token refresh with refresh token validation
    Err(StatusCode::NOT_IMPLEMENTED)
}
