//! Workspace member management — invite, list, remove, and update roles.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;
use super::{require_admin, resolve_workspace_id};

#[derive(Serialize)]
struct MemberResponse {
    user_id: i64,
    email: String,
    role: String,
    joined_at: String,
}

#[derive(Deserialize)]
struct InvitePayload {
    email: String,
    role: String,
}

#[derive(Deserialize)]
struct UpdateRolePayload {
    role: String,
}

pub(super) fn members_routes() -> Router<AppState> {
    Router::new()
        .route("/{slug}/invite", post(invite_member))
        .route("/{slug}/members", get(list_members))
        .route("/{slug}/members/{user_id}", delete(remove_member).patch(update_member_role))
}

async fn invite_member(
    auth_user: AuthUser,
    Path(slug): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<InvitePayload>,
) -> Result<impl IntoResponse, ApiError> {
    let ws_id = resolve_workspace_id(&state, &slug, auth_user.user_id).await?;
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
    let ws_id = resolve_workspace_id(&state, &slug, auth_user.user_id).await?;
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
    let ws_id = resolve_workspace_id(&state, &slug, auth_user.user_id).await?;
    require_admin(&state, ws_id, auth_user.user_id).await?;

    if target_user_id == auth_user.user_id {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Cannot remove yourself from the workspace",
        ));
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

async fn update_member_role(
    auth_user: AuthUser,
    Path((slug, target_user_id)): Path<(String, i64)>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateRolePayload>,
) -> Result<impl IntoResponse, ApiError> {
    let ws_id = resolve_workspace_id(&state, &slug, auth_user.user_id).await?;
    require_admin(&state, ws_id, auth_user.user_id).await?;

    if target_user_id == auth_user.user_id {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Cannot change your own role"));
    }

    if payload.role != "admin" && payload.role != "member" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Invalid role. Must be 'admin' or 'member'",
        ));
    }

    let result = sqlx::query(
        "UPDATE workspace_members SET role = $1 WHERE workspace_id = $2 AND user_id = $3",
    )
    .bind(&payload.role)
    .bind(ws_id)
    .bind(target_user_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Member not found"));
    }

    Ok(StatusCode::OK)
}
