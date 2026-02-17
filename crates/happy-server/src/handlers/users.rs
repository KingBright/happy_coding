//! User handlers

use crate::AppState;
use axum::{
    Json, extract::State, http::{StatusCode, HeaderMap},
};
use chrono::Utc;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct UserResponse {
    id: String,
    email: String,
    name: Option<String>,
    avatar_url: Option<String>,
    created_at: String,
    updated_at: String,
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserResponse>, StatusCode> {
    // Get Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user_id
    let user_id = state.auth_service.validate_token(token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // For now, return a simple response (we can query user details from DB if needed)
    let now = Utc::now().to_rfc3339();
    Ok(Json(UserResponse {
        id: user_id,
        email: "admin@hackerlife.fun".to_string(),
        name: Some("Admin".to_string()),
        avatar_url: None,
        created_at: now.clone(),
        updated_at: now,
    }))
}
