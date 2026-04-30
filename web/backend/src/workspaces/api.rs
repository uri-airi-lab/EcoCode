//! Workspace CRUD HTTP handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use tracing::{info, warn};
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;
use super::members::members_routes;
use super::{require_admin, resolve_workspace_id};

#[derive(Deserialize)]
struct CreateWorkspacePayload {
    name: String,
    slug: String,
}

#[derive(Serialize)]
struct WorkspaceResponse {
    id: i64,
    name: String,
    slug: String,
    role: String,
}

/// Mounts all workspace routes (CRUD + member sub-routes).
pub(crate) fn workspaces_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_workspace).get(list_workspaces))
        .route("/{slug}", delete(delete_workspace))
        .merge(members_routes())
}

async fn create_workspace(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateWorkspacePayload>,
) -> Result<impl IntoResponse, ApiError> {
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;

    // Check slug uniqueness within this user's workspaces only
    let slug_taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM workspaces w
           JOIN workspace_members m ON m.workspace_id = w.id
           WHERE w.slug = $1 AND m.user_id = $2
         )"
    )
    .bind(&payload.slug)
    .bind(auth_user.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    if slug_taken {
        return Err(ApiError::new(StatusCode::CONFLICT, "You already have a workspace with this slug"));
    }

    let res = sqlx::query("INSERT INTO workspaces (name, slug) VALUES ($1, $2) RETURNING id")
        .bind(&payload.name)
        .bind(&payload.slug)
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let workspace_id: i64 = res.get("id");

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(workspace_id)
    .bind(auth_user.user_id)
    .bind("admin")
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    tx.commit().await.map_err(ApiError::internal)?;
    info!(workspace_id, slug = %payload.slug, user_id = auth_user.user_id, "Workspace created");

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceResponse {
            id: workspace_id,
            name: payload.name,
            slug: payload.slug,
            role: "admin".to_string(),
        }),
    ))
}

async fn list_workspaces(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let workspaces = sqlx::query(
        "SELECT w.id, w.name, w.slug, m.role
         FROM workspaces w
         JOIN workspace_members m ON w.id = m.workspace_id
         WHERE m.user_id = $1",
    )
    .bind(auth_user.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .into_iter()
    .map(|r| WorkspaceResponse {
        id: r.get("id"),
        name: r.get("name"),
        slug: r.get("slug"),
        role: r.get("role"),
    })
    .collect::<Vec<_>>();

    Ok(Json(workspaces))
}

async fn delete_workspace(
    auth_user: AuthUser,
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let ws_id = resolve_workspace_id(&state, &slug, auth_user.user_id).await?;
    require_admin(&state, ws_id, auth_user.user_id).await?;

    let result = sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    if result.rows_affected() == 0 {
        warn!(slug = %slug, user_id = auth_user.user_id, "Delete workspace: not found");
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Workspace not found"));
    }

    info!(slug = %slug, user_id = auth_user.user_id, "Workspace deleted");
    Ok(StatusCode::NO_CONTENT)
}
