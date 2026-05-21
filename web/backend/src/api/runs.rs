//! Run API handlers — list, retrieve, and delete monitoring runs and their records.
//!
//! Record points include nested GPU and socket details. The `merge_record_points`
//! helper assembles flat database rows into the nested `RecordPoint` structures
//! expected by the frontend.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::auth;
use crate::error::ApiError;
use crate::models::record::{GpuRecordPoint, RecordPoint, RecordPointRow, SocketRecordPoint};
use crate::state::AppState;

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
    pub exporter_type: Option<String>,
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
    pub limit:    Option<i64>,
    pub offset:   Option<i64>,
    /// Unix milliseconds — inclusive lower bound for time-range queries (zoom window).
    pub start_ts: Option<i64>,
    /// Unix milliseconds — inclusive upper bound for time-range queries (zoom window).
    pub end_ts:   Option<i64>,
}

/// Selects up to `threshold` indices from `values` using the LTTB algorithm,
/// preserving the visual shape of the series (peaks, valleys, transitions).
fn lttb_indices(values: &[f64], threshold: usize) -> Vec<usize> {
    let n = values.len();
    if n <= threshold {
        return (0..n).collect();
    }

    let mut sampled = vec![0usize];
    let bucket_size = (n - 2) as f64 / (threshold - 2) as f64;
    let mut a = 0usize;

    for i in 0..(threshold - 2) {
        let cur_start = ((i + 1) as f64 * bucket_size) as usize + 1;
        let cur_end   = (((i + 2) as f64 * bucket_size) as usize + 1).min(n - 1);
        let nxt_start = cur_end;
        let nxt_end   = (((i + 3) as f64 * bucket_size) as usize + 1).min(n);

        let nxt_len = (nxt_end - nxt_start).max(1) as f64;
        let avg_x: f64 = (nxt_start..nxt_end).map(|j| j as f64).sum::<f64>() / nxt_len;
        let avg_y: f64 = (nxt_start..nxt_end).map(|j| values[j]).sum::<f64>() / nxt_len;

        let a_y = values[a];
        let mut max_area = -1.0f64;
        let mut max_idx  = cur_start;
        for j in cur_start..cur_end {
            let area = ((a as f64 - avg_x) * (values[j] - a_y)
                       - (a as f64 - j as f64) * (avg_y - a_y)).abs()
                / 2.0;
            if area > max_area {
                max_area = area;
                max_idx  = j;
            }
        }
        sampled.push(max_idx);
        a = max_idx;
    }

    sampled.push(n - 1);
    sampled
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

    debug!(
        limit,
        offset,
        user_id = auth_user.user_id,
        "Fetching all runs with summaries"
    );

    let runs_with_summaries = sqlx::query_as::<_, RunSummaryResponse>(
        "SELECT r.id, r.name, r.project_id, p.name as project_name, r.collaborator_name, r.status,
                r.started_at, r.ended_at, r.command, r.exporter_type,
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

    debug!(
        project_id,
        limit,
        offset,
        user_id = auth_user.user_id,
        "Fetching runs paginated"
    );

    // Get total count
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) 
         FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.project_id = $1 AND wm.user_id = $2",
    )
    .bind(project_id)
    .bind(auth_user.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    // Get paginated runs with summaries
    let runs = sqlx::query_as::<_, RunSummaryResponse>(
        "SELECT r.id, r.name, r.project_id, p.name as project_name, r.collaborator_name, r.status,
                r.started_at, r.ended_at, r.command, r.exporter_type,
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

    Ok((StatusCode::OK, Json(PaginatedRunsResponse { runs, total })))
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
///
/// Query params:
///   - `limit`    — max points to return (default 1000, max 5000)
///   - `offset`   — pagination offset (default 0)
///   - `start_ts` — Unix ms lower bound (zoom window; triggers LTTB path)
///   - `end_ts`   — Unix ms upper bound (zoom window; triggers LTTB path)
///
/// When `start_ts`/`end_ts` are present the handler returns LTTB-sampled records
/// within that window at full available resolution (up to `limit` points).
/// When absent, the existing paginated/step-sampled path is used.
pub async fn get_record_points(
    auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workspace membership
    let membership_check = sqlx::query(
        "SELECT 1 FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.id = $1 AND wm.user_id = $2",
    )
    .bind(run_id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    if membership_check.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found or access denied"));
    }
    let limit = pagination.limit.unwrap_or(1000).min(5000);
    let offset = pagination.offset.unwrap_or(0);

    // ── Zoom-window path ────────────────────────────────────────────────────────
    // When the frontend requests a specific time window (after zooming in) we
    // fetch all (id, total_energy) rows in that range, run LTTB to select the
    // most visually representative subset, then return full records for those IDs.
    if pagination.start_ts.is_some() || pagination.end_ts.is_some() {
        let start_ms = pagination.start_ts.unwrap_or(0);
        let end_ms   = pagination.end_ts.unwrap_or(i64::MAX);
        let max_pts  = limit as usize;

        debug!(run_id, start_ms, end_ms, max_pts, "Zoom-window record fetch (LTTB)");

        // Step 1: lightweight query — just id + total energy for LTTB selection
        let energy_rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT id,
                    COALESCE(cpu_energy, 0.0) + COALESCE(gpu_energy, 0.0) +
                    COALESCE(mem_energy, 0.0) + COALESCE(igpu_energy, 0.0) AS total_energy
             FROM records
             WHERE run_id = $1
               AND (EXTRACT(EPOCH FROM timestamp) * 1000)::bigint BETWEEN $2 AND $3
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(run_id)
        .bind(start_ms)
        .bind(end_ms)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::internal)?;

        if energy_rows.is_empty() {
            return Err(ApiError::new(StatusCode::NOT_FOUND, "No record points found for this run"));
        }

        // Step 2: LTTB — select at most max_pts indices
        let values: Vec<f64> = energy_rows.iter().map(|r| r.1).collect();
        let indices: Vec<usize> = lttb_indices(&values, max_pts);
        let selected_ids: Vec<i64> = indices.iter().map(|&i| energy_rows[i].0).collect();

        // Step 3: fetch full records for the selected IDs
        let rows = sqlx::query_as::<_, RecordPointRow>(
            "SELECT id, run_id, pid, timestamp::text AS timestamp, elapsed_secs,
                    cpu_usage, cpu_energy, gpu_usage, gpu_energy,
                    mem_usage, mem_energy, igpu_usage, igpu_energy,
                    phase, epoch_num, tokens_processed, training_step, loss,
                    learning_rate, sequence_length, accuracy, custom_metrics
             FROM records WHERE id = ANY($1)
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(&selected_ids)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::internal)?;

        let record_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        let (gpu_records, socket_records) = tokio::join!(
            crate::db::records::fetch_gpu_records_for_records(&state, &record_ids),
            crate::db::records::fetch_socket_records_for_records(&state, &record_ids),
        );

        let record_points = merge_record_points(rows, gpu_records?, socket_records?);
        debug!(run_id, returned_count = record_points.len(), "Zoom-window fetch complete");
        return Ok((StatusCode::OK, Json(record_points)));
    }

    // ── Standard paginated path ─────────────────────────────────────────────────
    debug!(run_id, limit, offset, "Fetching record points");

    let record_count: Option<i64> =
        sqlx::query_scalar("SELECT COUNT(*) FROM records WHERE run_id = $1")
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

    // For large datasets use LTTB — better shape preservation than step-based sampling
    let rows = if count > 10000 {
        let sample_limit = limit.min(2000) as usize;

        debug!(run_id, count, sample_limit, "Using LTTB sampling for large dataset");

        // Lightweight fetch for LTTB selection
        let energy_rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT id,
                    COALESCE(cpu_energy, 0.0) + COALESCE(gpu_energy, 0.0) +
                    COALESCE(mem_energy, 0.0) + COALESCE(igpu_energy, 0.0) AS total_energy
             FROM records WHERE run_id = $1
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(run_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::internal)?;

        let values: Vec<f64> = energy_rows.iter().map(|r| r.1).collect();
        let indices = lttb_indices(&values, sample_limit);
        let selected_ids: Vec<i64> = indices.iter().map(|&i| energy_rows[i].0).collect();

        sqlx::query_as::<_, RecordPointRow>(
            "SELECT id, run_id, pid, timestamp::text AS timestamp, elapsed_secs,
                    cpu_usage, cpu_energy, gpu_usage, gpu_energy,
                    mem_usage, mem_energy, igpu_usage, igpu_energy,
                    phase, epoch_num, tokens_processed, training_step, loss,
                    learning_rate, sequence_length, accuracy, custom_metrics
             FROM records WHERE id = ANY($1)
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(&selected_ids)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::internal)?
    } else {
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

    debug!(
        run_id,
        returned_count = record_points.len(),
        "Record points fetched successfully"
    );
    Ok((StatusCode::OK, Json(record_points)))
}

/// Handles GET requests to retrieve GPU-specific metrics for a run.
pub async fn get_gpu_records(
    auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workspace membership
    let membership_check = sqlx::query(
        "SELECT 1 FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.id = $1 AND wm.user_id = $2",
    )
    .bind(run_id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    if membership_check.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found or access denied"));
    }

    let gpu_records = crate::db::records::fetch_gpu_records_for_run(&state, run_id).await?;
    Ok((StatusCode::OK, Json(gpu_records)))
}

/// Handles GET requests to retrieve Socket-specific metrics for a run.
pub async fn get_socket_records(
    auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workspace membership
    let membership_check = sqlx::query(
        "SELECT 1 FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.id = $1 AND wm.user_id = $2",
    )
    .bind(run_id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    if membership_check.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found or access denied"));
    }

    let socket_records = crate::db::records::fetch_socket_records_for_run(&state, run_id).await?;
    Ok((StatusCode::OK, Json(socket_records)))
}

/// Handles GET requests to retrieve the aggregated summary of a run.
pub async fn get_run_summary(
    auth_user: auth::AuthUser,
    Path(run_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workspace membership
    let membership_check = sqlx::query(
        "SELECT 1 FROM runs r
         JOIN projects p ON r.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE r.id = $1 AND wm.user_id = $2",
    )
    .bind(run_id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    if membership_check.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found or access denied"));
    }

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
            warn!(
                run_id,
                user_id = auth_user.user_id,
                "Delete run denied: not found or no access"
            );
            return Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "Run not found or access denied",
            ));
        }
        Some(row) => {
            use sqlx::Row;
            let role: String = row.get("role");
            if role != "admin" {
                warn!(run_id, user_id = auth_user.user_id, role = %role, "Delete run denied: not an admin");
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "Only workspace admins can delete runs",
                ));
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
            elapsed_secs: row.elapsed_secs,
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
            phase: row.phase,
            epoch_num: row.epoch_num,
            tokens_processed: row.tokens_processed,
            training_step: row.training_step,
            loss: row.loss,
            learning_rate: row.learning_rate,
            sequence_length: row.sequence_length,
            accuracy: row.accuracy,
            custom_metrics: row.custom_metrics,
        })
        .collect()
}
