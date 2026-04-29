//! Run API handlers — list, retrieve, and delete monitoring runs and their records.
//!
//! Record points include nested GPU and socket details. The `merge_record_points`
//! helper assembles flat database rows into the nested `RecordPoint` structures
//! expected by the frontend.

use axum::{extract::{Path, Query, State}, http::StatusCode, Json, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::error::ApiError;
use crate::models::record::{RecordPoint, RecordPointRow, GpuRecordPoint, SocketRecordPoint};
use crate::state::AppState;
use crate::auth;

/// Optimized run summary for list views
#[derive(Serialize, sqlx::FromRow)]
pub struct RunSummaryResponse {
    pub id: i64,
    pub name: String,
    pub project_id: i64,
    pub project_name: Option<String>,
    pub collaborator_name: Option<String>,
    pub status: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub command: Option<String>,
    pub total_cpu_energy: Option<f64>,
    pub total_gpu_energy: Option<f64>,
    pub total_mem_energy: Option<f64>,
    pub total_igpu_energy: Option<f64>,
    pub total_co2_g: Option<f64>,
    pub carbon_intensity_used: Option<f64>,
    pub pue_used: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency_used: Option<String>,
    pub avg_cpu_usage: Option<f64>,
    pub avg_gpu_usage: Option<f64>,
    pub avg_mem_usage: Option<f64>,
}

/// Query parameters for paginated endpoints.
#[derive(Deserialize, Default)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Paginated response for runs
#[derive(Serialize)]
pub struct PaginatedRunsResponse {
    pub runs: Vec<RunSummaryResponse>,
    pub total: i64,
}

/// Handles GET requests to list all runs across all projects with summaries (optimized).
pub async fn get_all_runs_with_summaries(
    auth_user: auth::AuthUser,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = pagination.limit.unwrap_or(100).min(500);
    let offset = pagination.offset.unwrap_or(0);
    
    debug!(limit, offset, user_id = auth_user.user_id, "Fetching all runs with summaries");
    
    let runs_with_summaries = sqlx::query_as::<_, RunSummaryResponse>(
        "SELECT r.id, r.name, r.project_id, p.name as project_name, r.collaborator_name, r.status,
                r.started_at, r.ended_at, r.command,
                rs.total_cpu_energy, rs.total_gpu_energy, rs.total_mem_energy, rs.total_igpu_energy,
                rs.total_co2_g, rs.carbon_intensity_used, rs.pue_used, rs.total_cost, rs.currency_used,
                rs.avg_cpu_usage, rs.avg_gpu_usage, rs.avg_mem_usage
         FROM runs r
         LEFT JOIN runsummary rs ON r.id = rs.run_id
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE wm.user_id = $1
         ORDER BY r.started_at DESC, r.id DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(auth_user.user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    if runs_with_summaries.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No runs found",
        ));
    }

    Ok((StatusCode::OK, Json(runs_with_summaries)))
}

/// Handles GET requests to list paginated runs for a specific project.
pub async fn get_runs(
    auth_user: auth::AuthUser,
    Path(project_id): Path<i64>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = pagination.limit.unwrap_or(10).min(100);
    let offset = pagination.offset.unwrap_or(0);
    
    debug!(project_id, limit, offset, user_id = auth_user.user_id, "Fetching runs paginated");
    
    // Get total count
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) 
         FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.project_id = $1 AND wm.user_id = $2"
    )
    .bind(project_id)
    .bind(auth_user.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    // Get paginated runs with summaries
    let runs = sqlx::query_as::<_, RunSummaryResponse>(
        "SELECT r.id, r.name, r.project_id, p.name as project_name, r.collaborator_name, r.status,
                r.started_at, r.ended_at, r.command,
                rs.total_cpu_energy, rs.total_gpu_energy, rs.total_mem_energy, rs.total_igpu_energy,
                rs.total_co2_g, rs.carbon_intensity_used, rs.pue_used, rs.total_cost, rs.currency_used,
                rs.avg_cpu_usage, rs.avg_gpu_usage, rs.avg_mem_usage
         FROM runs r
         LEFT JOIN runsummary rs ON r.id = rs.run_id
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.project_id = $1 AND wm.user_id = $2
         ORDER BY r.started_at DESC, r.id DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(project_id)
    .bind(auth_user.user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok((StatusCode::OK, Json(PaginatedRunsResponse {
        runs,
        total,
    })))
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

/// Handles GET requests to retrieve record points for a run.
/// Supports `?limit=N&offset=M` pagination (default limit 1000, max 5000).
/// Optimized for Azure Container Apps with better timeout handling.
pub async fn get_record_points(
    Path(run_id): Path<i64>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = pagination.limit.unwrap_or(1000).min(5000);
    let offset = pagination.offset.unwrap_or(0);

    debug!(run_id, limit, offset, "Fetching record points");

    // First check if run exists and get record count
    let record_count: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM records WHERE run_id = $1"
    )
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let count = record_count.unwrap_or(0);
    if count == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No record points found for this run",
        ));
    }

    // For large datasets, use a more efficient query strategy
    let rows = if count > 10000 {
        // For very large runs, sample the data more aggressively
        let sample_limit = limit.min(2000);
        let step = (count as f64 / sample_limit as f64).ceil() as i64;
        
        debug!(run_id, count, sample_limit, step, "Using sampling strategy for large dataset");
        
        sqlx::query_as::<_, crate::models::record::RecordPointRow>(
            "SELECT * FROM (
                SELECT *, ROW_NUMBER() OVER (ORDER BY timestamp) as rn
                FROM records 
                WHERE run_id = $1
            ) t 
            WHERE rn % $2 = 1 
            ORDER BY timestamp 
            LIMIT $3 OFFSET $4"
        )
        .bind(run_id)
        .bind(step)
        .bind(sample_limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::internal)?
    } else {
        // Normal query for smaller datasets
        crate::db::records::fetch_record_rows_paginated(&state, run_id, limit, offset).await?
    };

    if rows.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No record points found for this run",
        ));
    }

    let record_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let (gpu_records, socket_records) = tokio::join!(
        crate::db::records::fetch_gpu_records_for_records(&state, &record_ids),
        crate::db::records::fetch_socket_records_for_records(&state, &record_ids),
    );

    let record_points = merge_record_points(rows, gpu_records?, socket_records?);
    
    debug!(run_id, returned_count = record_points.len(), "Record points fetched successfully");
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

/// Handles DELETE requests to delete a run.
pub async fn delete_run(
    auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if user has access to this run's project workspace
    let workspace_check = sqlx::query(
        "SELECT p.workspace_id, wm.role 
         FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.id = $1 AND wm.user_id = $2",
    )
    .bind(run_id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    match workspace_check {
        None => {
            warn!(run_id, user_id = auth_user.user_id, "Delete run denied: not found or no access");
            return Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found or access denied"));
        }
        Some(row) => {
            use sqlx::Row;
            let role: String = row.get("role");
            if role != "admin" {
                warn!(run_id, user_id = auth_user.user_id, role = %role, "Delete run denied: not an admin");
                return Err(ApiError::new(StatusCode::FORBIDDEN, "Only workspace admins can delete runs"));
            }
        }
    }

    // Delete the run (CASCADE will handle records)
    let result = sqlx::query("DELETE FROM runs WHERE id = $1")
        .bind(run_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found"));
    }

    info!(run_id, user_id = auth_user.user_id, "Run deleted");
    Ok(StatusCode::NO_CONTENT)
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
