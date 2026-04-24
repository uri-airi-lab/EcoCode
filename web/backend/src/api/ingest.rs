use axum::{extract::{Path, State}, http::StatusCode, Json, response::IntoResponse};
use serde_json::json;

use crate::error::ApiError;
use crate::models::ingest::{IngestRunPayload, IngestRecordData, IngestSystemInfo, IngestCompletePayload};
use crate::state::AppState;
use crate::auth;

/// Handles POST requests from the EcoCode CLI to register a new tracking run.
/// Validates workspace access and links the run to the authenticated user.
pub async fn ingest_run(
    auth_user: auth::AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<IngestRunPayload>,
) -> Result<impl IntoResponse, ApiError> {
    use sqlx::Row;
    
    let project_workspace = sqlx::query("SELECT workspace_id FROM projects WHERE id = $1")
        .bind(payload.project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Project not found"))?;

    let ws_id: i64 = project_workspace.get("workspace_id");
    
    let membership = sqlx::query("SELECT 1 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(ws_id)
        .bind(auth_user.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    if membership.is_none() {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Access denied to project workspace"));
    }

    let user_row = sqlx::query("SELECT email FROM users WHERE id = $1")
        .bind(auth_user.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let user_email: String = user_row.get("email");

    let run_id: i64 = sqlx::query_scalar(
        "INSERT INTO runs (name, project_id, collaborator_name, status, started_at, command)
         VALUES ($1, $2, $3, 'running', $4, $5)
         RETURNING id",
    )
    .bind(&payload.name)
    .bind(payload.project_id)
    .bind(user_email)
    .bind(payload.started_at)
    .bind(payload.command)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok((StatusCode::CREATED, Json(json!({ "id": run_id }))))
}

/// Handles POST requests from the EcoCode CLI to append a new time-series metrics record.
/// Saves CPU, GPU, Memory, and nested socket details into the database transactionally.
pub async fn ingest_record(
    _auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<IngestRecordData>,
) -> Result<impl IntoResponse, ApiError> {
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;

    let record_id: i64 = sqlx::query_scalar(
        "INSERT INTO records
            (run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy)
         VALUES ($1, $2, $3::timestamptz, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id",
    )
    .bind(run_id)
    .bind(payload.pid)
    .bind(&payload.timestamp)
    .bind(payload.cpu_usage)
    .bind(payload.cpu_energy)
    .bind(payload.gpu_usage)
    .bind(payload.gpu_energy)
    .bind(payload.mem_usage)
    .bind(payload.mem_energy)
    .bind(payload.igpu_usage)
    .bind(payload.igpu_energy)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    for gm in payload.gpu_metrics {
        sqlx::query("INSERT INTO gpu_records (record_id, device_index, gpu_usage, gpu_energy) VALUES ($1, $2, $3, $4)")
            .bind(record_id)
            .bind(gm.device_index)
            .bind(gm.gpu_usage)
            .bind(gm.gpu_energy)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    }

    for sm in payload.socket_metrics {
        sqlx::query("INSERT INTO socket_records (record_id, socket, domain, energy) VALUES ($1, $2, $3, $4)")
            .bind(record_id)
            .bind(sm.socket)
            .bind(&sm.domain)
            .bind(sm.energy)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    }

    tx.commit().await.map_err(ApiError::internal)?;
    Ok(StatusCode::CREATED)
}

/// Handles POST requests to store static hardware system information associated with a run.
pub async fn ingest_system_info(
    _auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<IngestSystemInfo>,
) -> Result<impl IntoResponse, ApiError> {
    sqlx::query(
        "INSERT INTO run_system_info (run_id, cpu_name, cpu_cores, ram_gb, gpu_count, gpu_names)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (run_id) DO UPDATE SET
            cpu_name = EXCLUDED.cpu_name,
            cpu_cores = EXCLUDED.cpu_cores,
            ram_gb = EXCLUDED.ram_gb,
            gpu_count = EXCLUDED.gpu_count,
            gpu_names = EXCLUDED.gpu_names",
    )
    .bind(run_id)
    .bind(&payload.cpu_name)
    .bind(payload.cpu_cores)
    .bind(payload.ram_gb)
    .bind(payload.gpu_count)
    .bind(&payload.gpu_names)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok(StatusCode::OK)
}

/// Handles POST requests to mark a run as finished.
/// Triggers the background calculation of the final footprint summary.
pub async fn ingest_complete_run(
    _auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<IngestCompletePayload>,
) -> Result<impl IntoResponse, ApiError> {
    sqlx::query("UPDATE runs SET status = $1, ended_at = $2 WHERE id = $3")
        .bind(&payload.status)
        .bind(&payload.ended_at)
        .bind(run_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    crate::db::runs::calculate_run_summary_db(run_id, &state.pool)
        .await
        .map_err(ApiError::internal)?;

    let _ = state.tx.send(format!("run_updated:{}", run_id));

    Ok(StatusCode::OK)
}
