//! Password hashing (Argon2) and API key hashing (SHA-256).

/// Returns `true` when the `PRODUCTION` environment variable is set to `"true"`.
/// Controls the `Secure` flag on auth cookies.
pub(super) fn is_production() -> bool {
    std::env::var("PRODUCTION").as_deref() != Ok("false")
}

/// Hashes an API key string with SHA-256 and returns the lowercase hex digest.
/// Only the hash is stored in the database; the plaintext key is shown once on creation.
pub(crate) fn hash_api_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}
