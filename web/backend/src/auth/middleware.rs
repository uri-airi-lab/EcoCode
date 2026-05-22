//! `AuthUser` request extractor — resolves the caller from a JWT cookie or API key header.

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, header, request::Parts},
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{DecodingKey, Validation, decode};
use sqlx::Row;

use super::jwt::{Claims, get_jwt_secret};
use super::password::hash_api_key;
use crate::error::ApiError;
use crate::state::AppState;
use tracing::{debug, warn};

pub struct AuthUser {
    pub user_id: i64,
    /// Set when authenticated via a workspace-scoped API key; `None` for JWT auth.
    pub workspace_id: Option<i64>,
    /// Set when authenticated via a project-scoped API key; `None` for JWT auth.
    pub project_id: Option<i64>,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
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
                            "SELECT user_id, workspace_id, project_id FROM api_keys WHERE key_hash = $1",
                        )
                        .bind(&key_hash)
                        .fetch_optional(&app_state.pool)
                        .await
                        .map_err(ApiError::internal)?;

                        if let Some(r) = row {
                            let user_id: i64 = r.get("user_id");
                            debug!(user_id, "Authenticated via API key");
                            return Ok(AuthUser {
                                user_id,
                                workspace_id: r.try_get("workspace_id").ok().flatten(),
                                project_id: r.try_get("project_id").ok().flatten(),
                            });
                        } else {
                            warn!("API key not found in database");
                        }
                    } else {
                        let secret = get_jwt_secret();
                        let decoding_key = DecodingKey::from_secret(secret.as_bytes());
                        if let Ok(token_data) =
                            decode::<Claims>(token_or_key, &decoding_key, &Validation::default())
                        {
                            debug!(
                                user_id = token_data.claims.sub,
                                "Authenticated via JWT (Bearer header)"
                            );
                            return Ok(AuthUser {
                                user_id: token_data.claims.sub,
                                workspace_id: None,
                                project_id: None,
                            });
                        } else {
                            warn!("Invalid JWT token in Authorization header");
                        }
                    }
                }
            }
        }

        let jar = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = jar.get("auth_token") {
            // CSRF protection for cookie-based authentication on state-changing methods
            if parts.method == axum::http::Method::POST
                || parts.method == axum::http::Method::PUT
                || parts.method == axum::http::Method::DELETE
            {
                let origin_header = parts.headers.get(axum::http::header::ORIGIN);
                let referer_header = parts.headers.get(axum::http::header::REFERER);

                let cors_origin = std::env::var("CORS_ORIGIN")
                    .unwrap_or_else(|_| "http://localhost:3000".to_string());
                let is_production = std::env::var("PRODUCTION")
                    .map(|v| v == "true")
                    .unwrap_or(false);

                let mut allowed_origins = vec![cors_origin];
                if !is_production {
                    allowed_origins.push("http://localhost:3000".to_string());
                    allowed_origins.push("http://127.0.0.1:3000".to_string());
                }

                let mut origin_valid = false;
                if let Some(origin_val) = origin_header {
                    if let Ok(origin_str) = origin_val.to_str() {
                        if allowed_origins.iter().any(|o| o == origin_str) {
                            origin_valid = true;
                        }
                    }
                } else if let Some(referer_val) = referer_header {
                    if let Ok(referer_str) = referer_val.to_str() {
                        if allowed_origins.iter().any(|o| referer_str.starts_with(o)) {
                            origin_valid = true;
                        }
                    }
                }

                if !origin_valid {
                    warn!("CSRF validation failed: missing or mismatched Origin/Referer for cookie auth");
                    return Err(ApiError::new(
                        StatusCode::FORBIDDEN,
                        "CSRF validation failed",
                    ));
                }
            }

            let secret = get_jwt_secret();
            let decoding_key = DecodingKey::from_secret(secret.as_bytes());
            if let Ok(token_data) =
                decode::<Claims>(cookie.value(), &decoding_key, &Validation::default())
            {
                debug!(
                    user_id = token_data.claims.sub,
                    "Authenticated via JWT cookie"
                );
                return Ok(AuthUser {
                    user_id: token_data.claims.sub,
                    workspace_id: None,
                    project_id: None,
                });
            } else {
                warn!("Invalid JWT token in auth_token cookie");
            }
        }

        warn!("Unauthenticated request — no valid credential found");
        Err(ApiError::new(StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}
