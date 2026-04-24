use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{post, get, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::state::AppState;
use crate::error::ApiError;
use crate::auth::AuthUser;

pub(crate) fn workspaces_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_workspace).get(list_workspaces))
        .route("/{slug}/invite", post(invite_member))
        .route("/{slug}/members", get(list_members))
        .route("/{slug}/members/{user_id}", delete(remove_member))
}

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

#[derive(Serialize)]
struct MemberResponse {
    user_id: i64,
    email: String,
    role: String,
    joined_at: String,
}

async fn create_workspace(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateWorkspacePayload>,
) -> Result<impl IntoResponse, ApiError> {
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;

    let res = sqlx::query("INSERT INTO workspaces (name, slug) VALUES ($1, $2) RETURNING id")
        .bind(&payload.name)
        .bind(&payload.slug)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate key value") {
                ApiError::new(StatusCode::CONFLICT, "Workspace slug already exists")
            } else {
                ApiError::internal(e)
            }
        })?;
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

#[derive(Deserialize)]
struct InvitePayload {
    email: String,
    role: String,
}

async fn invite_member(
    auth_user: AuthUser,
    Path(slug): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<InvitePayload>,
) -> Result<impl IntoResponse, ApiError> {
    let ws_id = resolve_workspace_id(&state, &slug).await?;
    require_admin(&state, ws_id, auth_user.user_id).await?;

    let target_user_row = sqlx::query("SELECT id FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    let target_user_id: i64 = match target_user_row {
        Some(r) => r.get("id"),
        None => return Err(ApiError::new(StatusCode::NOT_FOUND, "User with that email not found")),
    };

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(ws_id)
    .bind(target_user_id)
    .bind(&payload.role)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key value") {
            ApiError::new(StatusCode::CONFLICT, "User is already a member")
        } else {
            ApiError::internal(e)
        }
    })?;

    Ok(StatusCode::OK)
}

async fn list_members(
    auth_user: AuthUser,
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let ws_id = resolve_workspace_id(&state, &slug).await?;
    require_admin(&state, ws_id, auth_user.user_id).await?;

    let members = sqlx::query(
        "SELECT wm.user_id, u.email, wm.role, wm.joined_at::text AS joined_at
         FROM workspace_members wm
         JOIN users u ON wm.user_id = u.id
         WHERE wm.workspace_id = $1
         ORDER BY wm.joined_at ASC",
    )
    .bind(ws_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .into_iter()
    .map(|r| MemberResponse {
        user_id: r.get("user_id"),
        email: r.get("email"),
        role: r.get("role"),
        joined_at: r.get("joined_at"),
    })
    .collect::<Vec<_>>();

    Ok(Json(members))
}

async fn remove_member(
    auth_user: AuthUser,
    Path((slug, target_user_id)): Path<(String, i64)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let ws_id = resolve_workspace_id(&state, &slug).await?;
    require_admin(&state, ws_id, auth_user.user_id).await?;

    if target_user_id == auth_user.user_id {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Cannot remove yourself from the workspace"));
    }

    sqlx::query(
        "DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws_id)
    .bind(target_user_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Helpers ---

async fn resolve_workspace_id(state: &AppState, slug: &str) -> Result<i64, ApiError> {
    let row = sqlx::query("SELECT id FROM workspaces WHERE slug = $1")
        .bind(slug)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    match row {
        Some(r) => Ok(r.get("id")),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "Workspace not found")),
    }
}

async fn require_admin(state: &AppState, workspace_id: i64, user_id: i64) -> Result<(), ApiError> {
    let role_row = sqlx::query(
        "SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    match role_row {
        Some(r) => {
            let role: String = r.get("role");
            if role != "admin" {
                Err(ApiError::new(StatusCode::FORBIDDEN, "Only admins can perform this action"))
            } else {
                Ok(())
            }
        }
        None => Err(ApiError::new(StatusCode::FORBIDDEN, "You are not a member of this workspace")),
    }
}
