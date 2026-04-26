//! EcoCode web backend — Axum-based REST API and SSE server.
//!
//! Responsibilities:
//! - Initialise and migrate the PostgreSQL schema on startup.
//! - Expose REST endpoints for authentication, workspaces, projects, runs,
//!   measurement records, carbon configuration, and data ingestion from the CLI.
//! - Stream live record updates to connected browser clients over Server-Sent Events.
//!
//! Listens on `0.0.0.0:3001`. CORS origin is configured via the `CORS_ORIGIN`
//! environment variable (defaults to `http://localhost:3000`).

use axum::{
    Router,
    routing::{get, post, delete},
    http::Method,
    http::header::{CONTENT_TYPE, AUTHORIZATION},
};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::info;

pub mod api;
pub mod auth;
pub mod db;
pub mod error;
pub mod models;
pub mod services;
pub mod state;
pub mod workspaces;

use crate::state::AppState;

/// Simple endpoint to verify the server is running.
async fn hello_world() -> &'static str {
    "Hello, World!"
}

/// Entry point for the EcoCode backend API.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await?;
    info!("PostgreSQL pool initialized");

    // Ensure all tables and columns exist
    db::schema::ensure_postgres_schema(&pool).await?;
    info!("Database schema verified");

    let (tx, _) = broadcast::channel::<String>(1000);
    let state = AppState {
        pool,
        tx: tx.clone(),
    };

    // Start background watcher for new records
    tokio::spawn(api::sse::watch_changes(state.clone()));

    let cors_origin = std::env::var("CORS_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let app = Router::new()
        .route("/api/hello", get(hello_world))
        .nest("/api/auth", auth::auth_routes())
        .nest("/api/workspaces", workspaces::workspaces_routes())
        .route("/api/projects", get(api::projects::get_projects).post(api::projects::post_projects))
        .route("/api/projects/{project_id}", delete(api::projects::delete_project))
        .route("/api/project/{project_id}/runs", get(api::runs::get_runs))
        .route("/api/run/{run_id}", get(api::runs::get_run).delete(api::runs::delete_run))
        .route("/api/run/{run_id}/record_points", get(api::runs::get_record_points))
        .route("/api/run/{run_id}/summary", get(api::runs::get_run_summary))
        .route("/api/run/{run_id}/gpu_records", get(api::runs::get_gpu_records))
        .route("/api/run/{run_id}/socket_records", get(api::runs::get_socket_records))
        .route("/api/admin/projects", get(api::admin::get_admin_projects))
        .route("/api/sse", get(api::sse::sse_handler))
        .route("/api/project/{project_id}/carbon-config", get(api::carbon::get_carbon_config_handler).put(api::carbon::put_carbon_config_handler))
        .route("/api/project/{project_id}/carbon-config/refresh", post(api::carbon::post_refresh_carbon_intensity))
        .route("/api/carbon-regions", get(api::carbon::get_carbon_regions_handler))
        .route("/api/ingest/run", post(api::ingest::ingest_run))
        .route("/api/ingest/run/{run_id}/record", post(api::ingest::ingest_record))
        .route("/api/ingest/run/{run_id}/system-info", post(api::ingest::ingest_system_info))
        .route("/api/ingest/run/{run_id}/complete", post(api::ingest::ingest_complete_run))
        .layer(
            CorsLayer::new()
                .allow_origin(cors_origin.parse::<axum::http::HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                .allow_headers([CONTENT_TYPE, AUTHORIZATION])
                .allow_credentials(true),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    info!("Listening on 0.0.0.0:3001");
    axum::serve(listener, app).await?;

    Ok(())
}
