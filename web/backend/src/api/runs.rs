use axum::{extract::{Path, Query, State}, http::StatusCode, Json, response::IntoResponse};
use serde::Deserialize;
use std::collections::HashMap;

use crate::error::ApiError;
use crate::models::record::{RecordPoint, RecordPointRow, GpuRecordPoint, SocketRecordPoint};
use crate::state::AppState;
use crate::auth;

/// Query parameters for paginated endpoints.
#[derive(Deserialize, Default)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Handles GET requests to list paginated runs for a specific project.
pub async fn get_runs(
    auth_user: auth::AuthUser,
    Path(project_id): Path<i64>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = pagination.limit.unwrap_or(100).min(500);
    let offset = pagination.offset.unwrap_or(0);
    let runs = crate::db::runs::fetch_runs(&state, project_id, auth_user.user_id, limit, offset).await?;

    if runs.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No runs found for this project",
        ));
    }

    Ok((StatusCode::OK, Json(runs)))
}

/// Handles GET requests to retrieve details of a specific run.
pub async fn get_run(
    auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let run = crate::db::runs::fetch_run(&state, run_id, auth_user.user_id).await?;

    match run {
        Some(run) => Ok((StatusCode::OK, Json(run)).into_response()),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found")),
    }
}

/// Handles GET requests to retrieve all record points associated with a run.
/// Merges top-level metrics with GPU and Socket metrics.
pub async fn get_record_points(
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let rows = crate::db::records::fetch_record_rows(&state, run_id).await?;

    if rows.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No record points found for this run",
        ));
    }

    let gpu_records = crate::db::records::fetch_gpu_records_for_run(&state, run_id).await?;
    let socket_records = crate::db::records::fetch_socket_records_for_run(&state, run_id).await?;

    let record_points = merge_record_points(rows, gpu_records, socket_records);
    Ok((StatusCode::OK, Json(record_points)))
}

/// Handles GET requests to retrieve GPU-specific metrics for a run.
pub async fn get_gpu_records(
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let gpu_records = crate::db::records::fetch_gpu_records_for_run(&state, run_id).await?;
    Ok((StatusCode::OK, Json(gpu_records)))
}

/// Handles GET requests to retrieve Socket-specific metrics for a run.
pub async fn get_socket_records(
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let socket_records = crate::db::records::fetch_socket_records_for_run(&state, run_id).await?;
    Ok((StatusCode::OK, Json(socket_records)))
}

/// Handles GET requests to retrieve the aggregated summary of a run.
pub async fn get_run_summary(
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let summary = crate::db::runs::fetch_run_summary(&state, run_id).await?;

    match summary {
        Some(summary) => Ok((StatusCode::OK, Json(summary)).into_response()),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found")),
    }
}

/// Helper to merge flat database rows into nested `RecordPoint` structs.
fn merge_record_points(
    rows: Vec<RecordPointRow>,
    gpu_records: Vec<GpuRecordPoint>,
    socket_records: Vec<SocketRecordPoint>,
) -> Vec<RecordPoint> {
    let mut gpu_by_record: HashMap<i64, Vec<GpuRecordPoint>> = HashMap::new();
    for record in gpu_records {
        gpu_by_record
            .entry(record.record_id)
            .or_default()
            .push(record);
    }

    let mut socket_by_record: HashMap<i64, Vec<SocketRecordPoint>> = HashMap::new();
    for record in socket_records {
        socket_by_record
            .entry(record.record_id)
            .or_default()
            .push(record);
    }

    rows.into_iter()
        .map(|row| RecordPoint {
            id: row.id,
            run_id: row.run_id,
            pid: row.pid,
            timestamp: row.timestamp,
            cpu_usage: row.cpu_usage,
            cpu_energy: row.cpu_energy,
            gpu_usage: row.gpu_usage,
            gpu_energy: row.gpu_energy,
            mem_usage: row.mem_usage,
            mem_energy: row.mem_energy,
            igpu_usage: row.igpu_usage,
            igpu_energy: row.igpu_energy,
            gpu_details: gpu_by_record.remove(&row.id).unwrap_or_default(),
            socket_details: socket_by_record.remove(&row.id).unwrap_or_default(),
        })
        .collect()
}
