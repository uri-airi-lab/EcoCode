//! JWT generation and validation.

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Header, EncodingKey};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Claims {
    pub sub: i64,
    pub exp: usize,
}

/// Returns the JWT signing secret from `JWT_SECRET`, falling back to a development placeholder.
pub(super) fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "SUPER_SECRET_CHANGE_ME_IN_PROD".to_string())
}

/// Generates a signed JWT encoding the user's ID with a 30-day expiry.
pub(super) fn generate_jwt(user_id: i64) -> Result<String, ApiError> {
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
