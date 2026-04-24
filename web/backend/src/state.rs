use sqlx::PgPool;
use tokio::sync::broadcast;

/// Shared application state injected into Axum handlers.
/// 
/// Contains:
/// - `pool`: PostgreSQL database connection pool.
/// - `tx`: Broadcast sender for SSE (Server-Sent Events) notifications.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: broadcast::Sender<String>,
}
