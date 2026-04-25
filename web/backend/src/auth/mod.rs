//! Authentication module — user registration, login, JWT sessions, and API key management.
//!
//! ## Authentication methods
//! - **JWT cookie**: A 30-day `auth_token` HttpOnly cookie issued on login/register.
//!   Validated via `Authorization: Bearer <token>` or the `auth_token` cookie.
//! - **API key**: Workspace-scoped tokens prefixed with `ecocode_`, hashed with SHA-256
//!   before storage. Intended for the CLI `online` exporter.

mod jwt;
mod middleware;
mod password;

pub use middleware::AuthUser;
pub(crate) use password::hash_api_key;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::error::ApiError;
use crate::state::AppState;
use jwt::generate_jwt;
use password::{hash_api_key as hash_key, is_production};

/// Mounts all authentication routes under `/api/auth`.
pub(crate) fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/change-password", post(change_password))
        .route("/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api-keys/{id}", delete(delete_api_key))
}

#[derive(Deserialize)]
struct AuthPayload {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    token: String,
    user_id: i64,
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<AuthPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(e))?
        .to_string();

    let res = sqlx::query(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id",
    )
    .bind(&payload.email)
    .bind(&password_hash)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key value") {
            ApiError::new(StatusCode::CONFLICT, "Email already exists")
        } else {
            ApiError::internal(e)
        }
    })?;

    let user_id: i64 = res.get("id");
    let token = generate_jwt(user_id)?;
    let jar = build_auth_cookie(token.clone());
    Ok((StatusCode::CREATED, jar, Json(AuthResponse { token, user_id })))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<AuthPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let row = sqlx::query("SELECT id, password_hash FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    let (user_id, hash) = match row {
        Some(r) => (r.get::<i64, _>("id"), r.get::<String, _>("password_hash")),
        None => return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Invalid credentials")),
    };

    let parsed_hash = PasswordHash::new(&hash)
        .map_err(|_| ApiError::internal("Invalid hash format in DB"))?;
    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let token = generate_jwt(user_id)?;
    let jar = build_auth_cookie(token.clone());
    Ok((StatusCode::OK, jar, Json(AuthResponse { token, user_id })))
}

async fn logout() -> impl IntoResponse {
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.make_removal();
    let jar = CookieJar::new().add(cookie);
    (StatusCode::OK, jar)
}

#[derive(Deserialize)]
struct ChangePasswordPayload {
    current_password: String,
    new_password: String,
}

async fn change_password(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<ChangePasswordPayload>,
) -> Result<impl IntoResponse, ApiError> {
    if payload.new_password.len() < 8 {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "New password must be at least 8 characters",
        ));
    }

    let row = sqlx::query("SELECT password_hash FROM users WHERE id = $1")
        .bind(auth_user.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "User not found"))?;

    let current_hash: String = row.get("password_hash");
    let parsed_hash = PasswordHash::new(&current_hash)
        .map_err(|_| ApiError::internal("Invalid hash format in DB"))?;

    if Argon2::default()
        .verify_password(payload.current_password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Current password is incorrect"));
    }

    let salt = SaltString::generate(&mut OsRng);
    let new_hash = Argon2::default()
        .hash_password(payload.new_password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(e))?
        .to_string();

    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&new_hash)
        .bind(auth_user.user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CreateApiKeyPayload {
    name: String,
    workspace_id: Option<i64>,
}

#[derive(Serialize)]
struct ApiKeyResponse {
    id: i64,
    name: String,
    key: Option<String>,
    workspace_id: Option<i64>,
    created_at: String,
}

async fn create_api_key(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateApiKeyPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let raw_key: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let api_key = format!("ecocode_{}", raw_key);
    let key_hash = hash_key(&api_key);

    let row = sqlx::query(
        "INSERT INTO api_keys (user_id, name, key_hash, workspace_id) VALUES ($1, $2, $3, $4) RETURNING id, workspace_id, created_at::text",
    )
    .bind(auth_user.user_id)
    .bind(&payload.name)
    .bind(&key_hash)
    .bind(payload.workspace_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyResponse {
            id: row.get("id"),
            name: payload.name,
            key: Some(api_key),
            workspace_id: row.try_get("workspace_id").ok().flatten(),
            created_at: row.get("created_at"),
        }),
    ))
}

async fn list_api_keys(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let keys = sqlx::query(
        "SELECT id, name, workspace_id, created_at::text FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth_user.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .into_iter()
    .map(|r| ApiKeyResponse {
        id: r.get("id"),
        name: r.get("name"),
        key: None,
        workspace_id: r.try_get("workspace_id").ok().flatten(),
        created_at: r.get("created_at"),
    })
    .collect::<Vec<_>>();

    Ok(Json(keys))
}

async fn delete_api_key(
    auth_user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth_user.user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;

    Ok(StatusCode::NO_CONTENT)
}

fn build_auth_cookie(token: String) -> CookieJar {
    let cookie = Cookie::build(("auth_token", token))
        .path("/")
        .http_only(true)
        .secure(is_production())
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();
    CookieJar::new().add(cookie)
}
