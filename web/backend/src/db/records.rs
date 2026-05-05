use crate::error::ApiError;
use crate::models::record::{RecordPointRow, GpuRecordPoint, SocketRecordPoint, RecordPoint};
use crate::state::AppState;

/// Fetches all top-level record rows for a specific run.
pub async fn fetch_record_rows(
    state: &AppState,
    run_id: i64,
) -> Result<Vec<RecordPointRow>, ApiError> {
    sqlx::query_as::<_, RecordPointRow>(
        "SELECT id, run_id, pid, timestamp::text AS timestamp, cpu_usage, cpu_energy,
                gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy,
                phase, tokens_processed, training_step, loss, learning_rate, sequence_length, accuracy, custom_metrics
         FROM records
         WHERE run_id = $1
         ORDER BY timestamp ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches a paginated window of record rows for a specific run.
pub async fn fetch_record_rows_paginated(
    state: &AppState,
    run_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<RecordPointRow>, ApiError> {
    sqlx::query_as::<_, RecordPointRow>(
        "SELECT id, run_id, pid, timestamp::text AS timestamp, cpu_usage, cpu_energy,
                gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy,
                phase, tokens_processed, training_step, loss, learning_rate, sequence_length, accuracy, custom_metrics
         FROM records
         WHERE run_id = $1
         ORDER BY timestamp ASC, id ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(run_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches all GPU sub-records for a specific run, joined across all its top-level records.
pub async fn fetch_gpu_records_for_run(
    state: &AppState,
    run_id: i64,
) -> Result<Vec<GpuRecordPoint>, ApiError> {
    sqlx::query_as::<_, GpuRecordPoint>(
        "SELECT gr.id, gr.record_id, gr.device_index, gr.gpu_usage, gr.gpu_energy
         FROM gpu_records gr
         INNER JOIN records r ON r.id = gr.record_id
         WHERE r.run_id = $1
         ORDER BY gr.record_id ASC, gr.device_index ASC",
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches GPU sub-records for a specific set of record IDs (used with paginated record windows).
pub async fn fetch_gpu_records_for_records(
    state: &AppState,
    record_ids: &[i64],
) -> Result<Vec<GpuRecordPoint>, ApiError> {
    if record_ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, GpuRecordPoint>(
        "SELECT id, record_id, device_index, gpu_usage, gpu_energy
         FROM gpu_records
         WHERE record_id = ANY($1)
         ORDER BY record_id ASC, device_index ASC",
    )
    .bind(record_ids)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches all CPU socket sub-records for a specific run, joined across all its top-level records.
pub async fn fetch_socket_records_for_run(
    state: &AppState,
    run_id: i64,
) -> Result<Vec<SocketRecordPoint>, ApiError> {
    sqlx::query_as::<_, SocketRecordPoint>(
        "SELECT sr.id, sr.record_id, sr.socket, sr.domain, sr.energy
         FROM socket_records sr
         INNER JOIN records r ON r.id = sr.record_id
         WHERE r.run_id = $1
         ORDER BY sr.record_id ASC, sr.socket ASC, sr.domain ASC",
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches CPU socket sub-records for a specific set of record IDs (used with paginated record windows).
pub async fn fetch_socket_records_for_records(
    state: &AppState,
    record_ids: &[i64],
) -> Result<Vec<SocketRecordPoint>, ApiError> {
    if record_ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, SocketRecordPoint>(
        "SELECT id, record_id, socket, domain, energy
         FROM socket_records
         WHERE record_id = ANY($1)
         ORDER BY record_id ASC, socket ASC, domain ASC",
    )
    .bind(record_ids)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches the absolutely most recent record with its associated GPU and Socket details.
/// Used primarily by the background SSE watcher.
pub async fn fetch_latest_record_with_details(
    state: &AppState,
) -> Result<Option<RecordPoint>, ApiError> {
    let latest_row = sqlx::query_as::<_, RecordPointRow>(
        "SELECT id, run_id, pid, timestamp::text AS timestamp, cpu_usage, cpu_energy,
                gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy,
                phase, tokens_processed, training_step, loss, learning_rate, sequence_length, accuracy, custom_metrics
         FROM records
         ORDER BY id DESC
         LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let Some(row) = latest_row else {
        return Ok(None);
    };

    let gpu_details = sqlx::query_as::<_, GpuRecordPoint>(
        "SELECT id, record_id, device_index, gpu_usage, gpu_energy
         FROM gpu_records
         WHERE record_id = $1",
    )
    .bind(row.id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let socket_details = sqlx::query_as::<_, SocketRecordPoint>(
        "SELECT id, record_id, socket, domain, energy
         FROM socket_records
         WHERE record_id = $1",
    )
    .bind(row.id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok(Some(RecordPoint {
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
        gpu_details,
        socket_details,
        phase: row.phase,
        tokens_processed: row.tokens_processed,
        training_step: row.training_step,
        loss: row.loss,
        learning_rate: row.learning_rate,
        sequence_length: row.sequence_length,
        accuracy: row.accuracy,
        custom_metrics: row.custom_metrics,
    }))
}

/// Fetches a specific record by ID with its associated GPU and Socket details.
pub async fn fetch_record_by_id_with_details(
    id: i64,
    state: &AppState,
) -> Result<Option<RecordPoint>, ApiError> {
    let row = sqlx::query_as::<_, RecordPointRow>(
        "SELECT id, run_id, pid, timestamp::text AS timestamp, cpu_usage, cpu_energy,
                gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy,
                phase, tokens_processed, training_step, loss, learning_rate, sequence_length, accuracy, custom_metrics
         FROM records
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let Some(row) = row else {
        return Ok(None);
    };

    let gpu_details = sqlx::query_as::<_, GpuRecordPoint>(
        "SELECT id, record_id, device_index, gpu_usage, gpu_energy
         FROM gpu_records
         WHERE record_id = $1",
    )
    .bind(row.id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let socket_details = sqlx::query_as::<_, SocketRecordPoint>(
        "SELECT id, record_id, socket, domain, energy
         FROM socket_records
         WHERE record_id = $1",
    )
    .bind(row.id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok(Some(RecordPoint {
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
        gpu_details,
        socket_details,
        phase: row.phase,
        tokens_processed: row.tokens_processed,
        training_step: row.training_step,
        loss: row.loss,
        learning_rate: row.learning_rate,
        sequence_length: row.sequence_length,
        accuracy: row.accuracy,
        custom_metrics: row.custom_metrics,
    }))
}
