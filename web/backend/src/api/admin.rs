use crate::error::ApiError;
use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

/// Handles GET requests to list all projects for administrative overview.
pub async fn get_admin_projects(
    auth_user: crate::auth::AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let is_admin: bool = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = $1")
        .bind(auth_user.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    if !is_admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Access denied: platform admin privileges required",
        ));
    }

    let projects = crate::db::projects::fetch_admin_projects(&state).await?;
    Ok((StatusCode::OK, Json(projects)))
}
