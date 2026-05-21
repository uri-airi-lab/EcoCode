use sqlx::PgPool;
use tokio::sync::broadcast;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Instant, Duration};

/// In-memory IP-based rate limiter
pub struct RateLimiter {
    pub requests: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
        }
    }

    pub fn check_rate_limit(&self, ip: &str) -> bool {
        let mut map = self.requests.lock().unwrap();
        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);

        let timestamps = map.entry(ip.to_string()).or_default();
        timestamps.retain(|&ts| ts > one_minute_ago);

        if timestamps.len() >= 10 {
            false
        } else {
            timestamps.push(now);
            true
        }
    }
}

/// Shared application state injected into Axum handlers.
///
/// Contains:
/// - `pool`: PostgreSQL database connection pool.
/// - `tx`: Broadcast sender for SSE (Server-Sent Events) notifications.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: broadcast::Sender<String>,
    pub db_url: String,
    pub rate_limiter: std::sync::Arc<RateLimiter>,
}
