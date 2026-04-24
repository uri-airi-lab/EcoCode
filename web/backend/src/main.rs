use axum::{
    Router,
    routing::{get, post},
    http::Method,
    http::header::{CONTENT_TYPE, AUTHORIZATION},
};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

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

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await?;
    println!("PostgreSQL pool initialized.");
    
    // Ensure all tables and columns exist
    db::schema::ensure_postgres_schema(&pool).await?;

    let (tx, _) = broadcast::channel::<String>(100);
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
        .route("/api/project/{project_id}/runs", get(api::runs::get_runs))
        .route("/api/run/{run_id}", get(api::runs::get_run))
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
    axum::serve(listener, app).await?;

    Ok(())
}
