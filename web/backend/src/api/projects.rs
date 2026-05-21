//! Project API handlers — CRUD operations for monitoring projects.
//!
//! Projects belong to workspaces. Only workspace members can list or create projects;
//! only workspace admins can delete them. On creation, the provided address (if any)
//! is geocoded so that a real-time carbon intensity can later be fetched via the
//! Electricity Maps API.

use crate::auth;
use crate::error::ApiError;
use crate::models::project::{CreateProjectPayload, Project};
use crate::services::carbon_api::geocode_address;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use tracing::{info, warn};

/// Handles GET requests to list all projects accessible by the authenticated user.
pub async fn get_projects(
    auth_user: auth::AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let projects = crate::db::projects::fetch_projects(&state, auth_user.user_id).await?;

    if projects.is_empty() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "No projects found"));
    }

    Ok((StatusCode::OK, Json(projects)))
}

/// Handles POST requests to create a new project within a specific workspace.
pub async fn post_projects(
    auth_user: auth::AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateProjectPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let membership =
        sqlx::query("SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
            .bind(payload.workspace_id)
            .bind(auth_user.user_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;

    if membership.is_none() {
        warn!(
            user_id = auth_user.user_id,
            workspace_id = payload.workspace_id,
            "Create project denied: user not a workspace member"
        );
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "You are not a member of this workspace",
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();

    let mut lat = None;
    let mut lon = None;
    let mut resolved_address = payload.address.clone();
    let intensity = payload.intensity.unwrap_or(445.0); // IEA 2025 global average fallback
    let intensity_src = if payload.intensity.is_some() {
        "manual".to_string()
    } else {
        "static_WORLD".to_string()
    };

    if let Some(addr) = &payload.address {
        if let Ok((l_lat, l_lon, display)) = geocode_address(addr).await {
            lat = Some(l_lat);
            lon = Some(l_lon);
            resolved_address = Some(display);
        }
    }

    let cost = payload.cost;
    let currency = payload.currency.unwrap_or_else(|| "USD".to_string());
    let use_api_for_cost = payload.use_api_for_cost.unwrap_or(false);

    let result = sqlx::query(
        "INSERT INTO projects (workspace_id, name, resolved_intensity_g_per_kwh, intensity_source, manual_cost_per_kwh, manual_currency, address, latitude, longitude, use_api_for_cost, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, name",
    )
    .bind(payload.workspace_id)
    .bind(&payload.name)
    .bind(intensity)
    .bind(intensity_src)
    .bind(cost)
    .bind(&currency)
    .bind(resolved_address)
    .bind(lat)
    .bind(lon)
    .bind(use_api_for_cost)
    .bind(&now)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key value") {
            ApiError::new(StatusCode::CONFLICT, "A project with this name already exists in this workspace")
        } else {
            ApiError::internal(e)
        }
    })?;

    use sqlx::Row;
    let id: i64 = result.get("id");
    let name: String = result.get("name");

    let slug_row = sqlx::query("SELECT slug FROM workspaces WHERE id = $1")
        .bind(payload.workspace_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let workspace_slug: String = slug_row.get("slug");

    info!(project_id = id, name = %name, workspace_id = payload.workspace_id, user_id = auth_user.user_id, "Project created");
    Ok((
        StatusCode::CREATED,
        Json(Project {
            id,
            name: Some(name),
            workspace_slug,
        }),
    ))
}

/// Handles DELETE requests to delete a project.
pub async fn delete_project(
    auth_user: auth::AuthUser,
    Path(project_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if user has access to this project's workspace
    let workspace_check = sqlx::query(
        "SELECT p.workspace_id, wm.role 
         FROM projects p
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         WHERE p.id = $1 AND wm.user_id = $2",
    )
    .bind(project_id)
    .bind(auth_user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    match workspace_check {
        None => {
            warn!(
                project_id,
                user_id = auth_user.user_id,
                "Delete project denied: not found or no access"
            );
            return Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "Project not found or access denied",
            ));
        }
        Some(row) => {
            use sqlx::Row;
            let role: String = row.get("role");
            if role != "admin" {
                warn!(project_id, user_id = auth_user.user_id, role = %role, "Delete project denied: not an admin");
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "Only workspace admins can delete projects",
                ));
            }
        }
    }

    // Delete the project (CASCADE will handle runs and records)
    let result = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(project_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Project not found"));
    }

    info!(project_id, user_id = auth_user.user_id, "Project deleted");
    Ok(StatusCode::NO_CONTENT)
}
