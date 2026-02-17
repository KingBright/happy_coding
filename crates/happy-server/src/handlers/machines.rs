//! Machine handlers

use crate::AppState;
use axum::{Json, extract::{State, Path}, http::StatusCode};
use happy_core::{Machine, MachineInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct MachineListResponse {
    machines: Vec<MachineInfo>,
}

pub async fn list(
    State(_state): State<AppState>,
) -> Result<Json<MachineListResponse>, StatusCode> {
    // TODO: Implement list machines
    Ok(Json(MachineListResponse { machines: vec![] }))
}

#[derive(Debug, Deserialize)]
pub struct RegisterMachineRequest {
    name: String,
    public_key: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct MachineResponse {
    machine: Machine,
}

pub async fn register(
    State(_state): State<AppState>,
    Json(_req): Json<RegisterMachineRequest>,
) -> Result<Json<MachineResponse>, StatusCode> {
    // TODO: Implement register machine
    Err(StatusCode::NOT_IMPLEMENTED)
}

pub async fn get(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
) -> Result<Json<MachineResponse>, StatusCode> {
    // TODO: Implement get machine
    Err(StatusCode::NOT_IMPLEMENTED)
}
