//! Workspace management — CRUD operations and member role management.
//!
//! A workspace is the top-level organisational unit. Each workspace has an admin
//! (the creator) and can have additional members. Projects belong to workspaces.

mod api;
mod members;

pub(crate) use api::workspaces_routes;

use axum::http::StatusCode;
use sqlx::Row;

use crate::error::ApiError;
use crate::state::AppState;

/// Looks up a workspace by slug within the authenticated user's memberships.
pub(super) async fn resolve_workspace_id(
    state: &AppState,
    slug: &str,
    user_id: i64,
) -> Result<i64, ApiError> {
    let row = sqlx::query(
        "SELECT w.id FROM workspaces w
         JOIN workspace_members m ON m.workspace_id = w.id
         WHERE w.slug = $1 AND m.user_id = $2",
    )
    .bind(slug)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    match row {
        Some(r) => Ok(r.get("id")),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "Workspace not found")),
    }
}

/// Returns `Ok(())` if the user holds the `admin` role in the workspace,
/// otherwise returns `403 Forbidden`.
pub(super) async fn require_admin(
    state: &AppState,
    workspace_id: i64,
    user_id: i64,
) -> Result<(), ApiError> {
    let role_row =
        sqlx::query("SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace_id)
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;

    match role_row {
        Some(r) => {
            let role: String = r.get("role");
            if role != "admin" {
                Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "Only admins can perform this action",
                ))
            } else {
                Ok(())
            }
        }
        None => Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "You are not a member of this workspace",
        )),
    }
}
