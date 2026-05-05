use crate::error::ApiError;
use crate::models::carbon::CarbonConfigRow;
use crate::state::AppState;
use axum::http::StatusCode;

/// Fetches the carbon footprint configuration settings associated with a specific project.
pub async fn get_project_carbon_config(
    state: &AppState,
    project_id: i64,
) -> Result<CarbonConfigRow, ApiError> {
    sqlx::query_as::<_, CarbonConfigRow>(
        "SELECT country_code, address, latitude, longitude, electricity_maps_api_key,
                resolved_intensity_g_per_kwh, intensity_source, intensity_fetched_at,
                pue_factor, manual_cost_per_kwh, manual_currency, use_api_for_cost, updated_at
         FROM projects WHERE id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("Project {project_id} not found"),
        )
    })
}
