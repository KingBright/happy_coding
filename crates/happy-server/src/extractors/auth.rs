//! Auth extractor for protected routes

use crate::AppState;
use axum::{
    extract::Extension,
    http::{StatusCode, Request, request::Parts},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

/// Authenticated user info
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub email: String,
}

/// Auth error response
pub struct AuthError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.message,
            "code": "unauthorized"
        }));
        (self.status, body).into_response()
    }
}

/// Extract auth user from request
pub async fn extract_auth_user<B>(
    parts: &mut Parts,
    app_state: &AppState,
) -> Result<AuthUser, AuthError> {
    // Get Authorization header
    let auth_header = parts
        .headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: "Missing Authorization header".to_string(),
        })?;

    // Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid Authorization format".to_string(),
        })?;

    // Validate token - returns user_id directly
    match app_state.auth_service.validate_token(token).await {
        Ok(user_id) => {
            // Get user info from database
            match app_state.db.get_user_by_id(&user_id).await {
                Ok(Some((id, email, _name))) => Ok(AuthUser {
                    user_id: id,
                    email,
                }),
                Ok(None) => Err(AuthError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "User not found".to_string(),
                }),
                Err(e) => Err(AuthError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    message: format!("Database error: {}", e),
                }),
            }
        }
        Err(e) => Err(AuthError {
            status: StatusCode::UNAUTHORIZED,
            message: format!("Invalid token: {}", e),
        }),
    }
}
