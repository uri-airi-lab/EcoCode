//! `AuthUser` request extractor — resolves the caller from a JWT cookie or API key header.

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use sqlx::Row;

use crate::error::ApiError;
use crate::state::AppState;
use super::jwt::{get_jwt_secret, Claims};
use super::password::hash_api_key;

pub struct AuthUser {
    pub user_id: i64,
    /// Set when authenticated via a workspace-scoped API key; `None` for JWT auth.
    pub workspace_id: Option<i64>,
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
                        if let Ok(token_data) =
                            decode::<Claims>(token_or_key, &decoding_key, &Validation::default())
                        {
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
            if let Ok(token_data) =
                decode::<Claims>(cookie.value(), &decoding_key, &Validation::default())
            {
                return Ok(AuthUser {
                    user_id: token_data.claims.sub,
                    workspace_id: None,
                });
            }
        }

        Err(ApiError::new(StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}
