use crate::error::ApiError;
use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

/// Handles GET requests to list all projects for administrative overview.
pub async fn get_admin_projects(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let projects = crate::db::projects::fetch_admin_projects(&state).await?;
    Ok((StatusCode::OK, Json(projects)))
}
