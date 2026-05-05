//! Database queries for runs and run summaries.
//!
//! Run queries join `run_system_info` (LEFT JOIN) and `workspace_members` to enforce
//! access control at the SQL level — users can only fetch runs they are authorised to see.
//! `calculate_run_summary_db` is invoked by the ingest completion endpoint to aggregate
//! all records into a single summary row with CO2 and cost figures.

use crate::error::ApiError;
use crate::models::run::{Run, RunSummary};
use crate::state::AppState;
use sqlx::PgPool;

/// Fetches a paginated list of runs for a given project, verifying the user's access.
pub async fn fetch_runs(
    state: &AppState,
    project_id: i64,
    user_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Run>, ApiError> {
    sqlx::query_as::<_, Run>(
        "SELECT runs.id, runs.name, runs.project_id, runs.collaborator_name, runs.status,
                runs.started_at, runs.ended_at, runs.command, runs.exporter_type,
                run_system_info.cpu_name, run_system_info.cpu_cores, run_system_info.ram_gb,
                run_system_info.gpu_count, run_system_info.gpu_names
         FROM runs
         LEFT JOIN run_system_info ON runs.id = run_system_info.run_id
         JOIN projects p ON runs.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE runs.project_id = $1 AND wm.user_id = $2
         ORDER BY runs.started_at DESC, runs.id DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(project_id)
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches a specific run by ID, verifying the user's access through the workspace.
pub async fn fetch_run(
    state: &AppState,
    run_id: i64,
    user_id: i64,
) -> Result<Option<Run>, ApiError> {
    sqlx::query_as::<_, Run>(
        "SELECT runs.id, runs.name, runs.project_id, runs.collaborator_name, runs.status,
                runs.started_at, runs.ended_at, runs.command, runs.exporter_type,
                run_system_info.cpu_name, run_system_info.cpu_cores, run_system_info.ram_gb,
                run_system_info.gpu_count, run_system_info.gpu_names
         FROM runs
         LEFT JOIN run_system_info ON runs.id = run_system_info.run_id
         JOIN projects p ON runs.project_id = p.id
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE runs.id = $1 AND wm.user_id = $2",
    )
    .bind(run_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches the aggregated run summary (total energy, CO2, and cost).
pub async fn fetch_run_summary(
    state: &AppState,
    run_id: i64,
) -> Result<Option<RunSummary>, ApiError> {
    sqlx::query_as::<_, RunSummary>(
        "SELECT run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy,
                total_co2_g, carbon_intensity_used, pue_used, total_cost, currency_used,
                avg_cpu_usage, avg_gpu_usage, avg_mem_usage
         FROM runsummary
         WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Computes the final run summary from all its records and the project's carbon configuration.
///
/// Sums up energy (Joules) across all components, converts to Watt-hours, and applies
/// the carbon intensity (gCO2/kWh) and energy cost to calculate the final footprint and expense.
pub async fn calculate_run_summary_db(
    run_id: i64,
    db: &PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let record_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM records WHERE run_id = $1")
        .bind(run_id)
        .fetch_one(db)
        .await?;

    sqlx::query("DELETE FROM runsummary WHERE run_id = $1")
        .bind(run_id)
        .execute(db)
        .await?;

    if record_count == 0 {
        return Ok(());
    }

    let config_row = sqlx::query(
        "SELECT p.resolved_intensity_g_per_kwh, p.pue_factor, p.intensity_source, p.manual_cost_per_kwh, p.manual_currency
         FROM projects p
         JOIN runs r ON r.project_id = p.id
         WHERE r.id = $1",
    )
    .bind(run_id)
    .fetch_optional(db)
    .await?;

    let (intensity, pue, _source, cost_per_kwh, currency) = if let Some(r) = config_row {
        use sqlx::Row;
        (
            r.get::<f64, _>("resolved_intensity_g_per_kwh"),
            r.get::<f64, _>("pue_factor"),
            r.get::<String, _>("intensity_source"),
            r.get::<Option<f64>, _>("manual_cost_per_kwh")
                .unwrap_or(0.0),
            r.get::<Option<String>, _>("manual_currency")
                .unwrap_or_else(|| "USD".to_string()),
        )
    } else {
        // IEA Emissions Factors 2025: global average = 445 gCO2/kWh
        (445.0, 1.0, "WORLD".to_string(), 0.0, "USD".to_string())
    };

    sqlx::query(
        "INSERT INTO runsummary
            (run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy,
             total_co2_g, carbon_intensity_used, pue_used, total_cost, currency_used,
             avg_cpu_usage, avg_gpu_usage, avg_mem_usage)
          SELECT
            $1,
            COALESCE(SUM(cpu_energy),  0.0),
            COALESCE(SUM(gpu_energy),  0.0),
            COALESCE(SUM(mem_energy),  0.0),
            COALESCE(SUM(igpu_energy), 0.0),
            0.0, $2, $3, 0.0, $4,
            COALESCE(AVG(cpu_usage), 0.0),
            COALESCE(AVG(gpu_usage), 0.0),
            COALESCE(AVG(mem_usage), 0.0)
          FROM records
          WHERE run_id = $1",
    )
    .bind(run_id)
    .bind(intensity)
    .bind(pue)
    .bind(&currency)
    .execute(db)
    .await?;

    let totals: (f64, f64, f64, f64) = sqlx::query_as(
        "SELECT total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy
         FROM runsummary WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_one(db)
    .await?;

    let energy_wh = (totals.0 + totals.1 + totals.2 + totals.3) / 3600.0;
    let co2_g = energy_wh / 1000.0 * intensity * pue;
    let total_cost = (energy_wh / 1000.0) * cost_per_kwh * pue;

    sqlx::query("UPDATE runsummary SET total_co2_g = $1, total_cost = $2 WHERE run_id = $3")
        .bind(co2_g)
        .bind(total_cost)
        .bind(run_id)
        .execute(db)
        .await?;

    Ok(())
}
