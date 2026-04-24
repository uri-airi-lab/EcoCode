use axum::{
    extract::{FromRequestParts, State, Path, FromRef},
    http::{request::Parts, StatusCode, header},
    response::IntoResponse,
    routing::{post, delete},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use rand::{distributions::Alphanumeric, Rng};
use chrono::{Utc, Duration};
use sqlx::Row;

use crate::state::AppState;
use crate::error::ApiError;

pub(crate) fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api-keys/{id}", delete(delete_api_key))
}

fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "SUPER_SECRET_CHANGE_ME_IN_PROD".to_string())
}

fn is_production() -> bool {
    std::env::var("PRODUCTION").as_deref() == Ok("true")
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i64,
    exp: usize,
}

pub struct AuthUser {
    pub user_id: i64,
    /// Set when authenticated via a workspace-scoped API key; None for JWT auth.
    pub workspace_id: Option<i64>,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token_or_key = &auth_str[7..];

                    if token_or_key.starts_with("ecocode_") {
                        let key_hash = hash_api_key(token_or_key);
                        let row = sqlx::query(
                            "SELECT user_id, workspace_id FROM api_keys WHERE key_hash = $1",
                        )
                        .bind(&key_hash)
                        .fetch_optional(&app_state.pool)
                        .await
                        .map_err(ApiError::internal)?;

                        if let Some(r) = row {
                            return Ok(AuthUser {
                                user_id: r.get("user_id"),
                                workspace_id: r.try_get("workspace_id").ok().flatten(),
                            });
                        }
                    } else {
                        let secret = get_jwt_secret();
                        let decoding_key = DecodingKey::from_secret(secret.as_bytes());
                        if let Ok(token_data) = decode::<Claims>(token_or_key, &decoding_key, &Validation::default()) {
                            return Ok(AuthUser {
                                user_id: token_data.claims.sub,
                                workspace_id: None,
                            });
                        }
                    }
                }
            }
        }

        let jar = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = jar.get("auth_token") {
            let secret = get_jwt_secret();
            let decoding_key = DecodingKey::from_secret(secret.as_bytes());
            if let Ok(token_data) = decode::<Claims>(cookie.value(), &decoding_key, &Validation::default()) {
                return Ok(AuthUser {
                    user_id: token_data.claims.sub,
                    workspace_id: None,
                });
            }
        }

        Err(ApiError::new(StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}

pub(crate) fn hash_api_key(key: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
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

    let cookie = Cookie::build(("auth_token", token.clone()))
        .path("/")
        .http_only(true)
        .secure(is_production())
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();

    let jar = CookieJar::new().add(cookie);
    Ok((StatusCode::CREATED, jar, Json(AuthResponse { token, user_id })))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<AuthPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let row = sqlx::query(
        "SELECT id, password_hash FROM users WHERE email = $1",
    )
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

    let cookie = Cookie::build(("auth_token", token.clone()))
        .path("/")
        .http_only(true)
        .secure(is_production())
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();

    let jar = CookieJar::new().add(cookie);
    Ok((StatusCode::OK, jar, Json(AuthResponse { token, user_id })))
}

async fn logout() -> impl IntoResponse {
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.make_removal();
    let jar = CookieJar::new().add(cookie);
    (StatusCode::OK, jar)
}

fn generate_jwt(user_id: i64) -> Result<String, ApiError> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(30))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user_id,
        exp: expiration as usize,
    };

    let secret = get_jwt_secret();
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| ApiError::internal(e))
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
    let key_hash = hash_api_key(&api_key);

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
