//! Data ingestion API — endpoints called by the EcoCode CLI (`online` exporter).
//!
//! The CLI authenticates via a workspace-scoped API key in the `Authorization: Bearer` header.
//! The ingestion flow is:
//! 1. `POST /api/ingest/run`             — create a run record (status = running).
//! 2. `POST /api/ingest/run/{id}/record` — append one measurement record per interval.
//! 3. `POST /api/ingest/run/{id}/system-info` — save hardware specs (once per run).
//! 4. `POST /api/ingest/run/{id}/complete`    — finalise the run and trigger summary calculation.
//!
//! All record insertions are transactional; GPU and socket breakdowns are inserted
//! in the same transaction as the parent record.

use axum::{extract::{Path, State}, http::StatusCode, Json, response::IntoResponse};
use serde_json::json;
use tracing::{debug, info, warn};

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
    
    // Resolve project_id: API key scope takes priority over payload value.
    let resolved_project_id = auth_user.project_id
        .or(payload.project_id)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "project_id is required when using a workspace-scoped API key"))?;

    let project_workspace = sqlx::query("SELECT workspace_id FROM projects WHERE id = $1")
        .bind(resolved_project_id)
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
        warn!(user_id = auth_user.user_id, project_id = payload.project_id, "Ingest run denied: user not a member of project workspace");
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Access denied to project workspace"));
    }

    let user_row = sqlx::query("SELECT email FROM users WHERE id = $1")
        .bind(auth_user.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let user_email: String = user_row.get("email");

    let run_id: i64 = sqlx::query_scalar(
        "INSERT INTO runs (name, project_id, collaborator_name, status, started_at, command, exporter_type)
         VALUES ($1, $2, $3, 'running', $4, $5, $6)
         RETURNING id",
    )
    .bind(&payload.name)
    .bind(resolved_project_id)
    .bind(user_email)
    .bind(payload.started_at)
    .bind(payload.command)
    .bind(payload.exporter_type)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    info!(run_id, project_id = payload.project_id, user_id = auth_user.user_id, name = %payload.name, "Run created");
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
            (run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy,
             mem_usage, mem_energy, igpu_usage, igpu_energy,
             phase, tokens_processed, training_step, loss, learning_rate, sequence_length, accuracy, custom_metrics)
         VALUES ($1, $2, $3::timestamptz, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
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
    .bind(&payload.phase)
    .bind(payload.tokens_processed)
    .bind(payload.training_step)
    .bind(payload.loss)
    .bind(payload.learning_rate)
    .bind(payload.sequence_length)
    .bind(payload.accuracy)
    .bind(&payload.custom_metrics)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    for gm in &payload.gpu_metrics {
        sqlx::query("INSERT INTO gpu_records (record_id, device_index, gpu_usage, gpu_energy) VALUES ($1, $2, $3, $4)")
            .bind(record_id)
            .bind(gm.device_index)
            .bind(gm.gpu_usage)
            .bind(gm.gpu_energy)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    }

    for sm in &payload.socket_metrics {
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
    debug!(run_id, record_id, "Record ingested");

    // Direct broadcast to internal SSE channel for immediate UI update
    let mut gpu_details = Vec::new();
    for gm in &payload.gpu_metrics {
        gpu_details.push(crate::models::record::GpuRecordPoint {
            id: 0,
            record_id,
            device_index: gm.device_index,
            gpu_usage: gm.gpu_usage,
            gpu_energy: gm.gpu_energy,
        });
    }

    let mut socket_details = Vec::new();
    for sm in &payload.socket_metrics {
        socket_details.push(crate::models::record::SocketRecordPoint {
            id: 0,
            record_id,
            socket: sm.socket,
            domain: sm.domain.clone(),
            energy: sm.energy,
        });
    }

    let record = crate::models::record::RecordPoint {
        id: record_id,
        run_id,
        pid: payload.pid,
        timestamp: payload.timestamp.clone(),
        cpu_usage: Some(payload.cpu_usage),
        cpu_energy: Some(payload.cpu_energy),
        gpu_usage: Some(payload.gpu_usage),
        gpu_energy: Some(payload.gpu_energy),
        mem_usage: Some(payload.mem_usage),
        mem_energy: payload.mem_energy,
        igpu_usage: payload.igpu_usage,
        igpu_energy: payload.igpu_energy,
        gpu_details,
        socket_details,
        phase: payload.phase.clone(),
        tokens_processed: payload.tokens_processed,
        training_step: payload.training_step,
        loss: payload.loss,
        learning_rate: payload.learning_rate,
        sequence_length: payload.sequence_length,
        accuracy: payload.accuracy,
        custom_metrics: payload.custom_metrics.clone(),
    };

    if let Ok(json_payload) = serde_json::to_string(&record) {
        let _ = state.tx.send(json_payload);
    }

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

    debug!(run_id, "System info stored");
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
    info!(run_id, status = %payload.status, "Run completed");

    Ok(StatusCode::OK)
}
