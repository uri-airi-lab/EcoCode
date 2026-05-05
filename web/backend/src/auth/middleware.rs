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
