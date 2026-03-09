use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    routing::{get},
    Json, Router,
};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqlitePoolOptions;
use std::{collections::HashMap, convert::Infallible, path::PathBuf, vec};
use tokio::time::Duration;
use tower_http::cors::{Any, CorsLayer};

// use axum_extra::TypedHeader;
use futures::stream::{Stream, StreamExt};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Serialize, Clone, sqlx::FromRow)]
struct Project {
    id: i64,
    name: String,
}
#[derive(Serialize, Clone, sqlx::FromRow)]
struct Run {
    id: i64,
    name: String,
    project_id: i64,
    collaborator_name: Option<String>,
    status: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
}

#[derive(Serialize, Clone, sqlx::FromRow)]
struct RecordPoint {
    id: i64,
    run_id: i64,
    pid: i64,
    timestamp: String,
    cpu_usage: f64,
    cpu_energy: f64,
    gpu_usage: f64,
    gpu_energy: f64,
    mem_usage: f64,
    mem_energy: f64,
    igpu_usage: f64,
    igpu_energy: f64,
}

#[derive(Serialize, Clone, sqlx::FromRow, Debug)]
struct RunSummary {
    run_id: i64,
    total_cpu_energy: f64,
    total_gpu_energy: f64,
    total_mem_energy: f64,
    total_igpu_energy: f64,
}

#[derive(Clone, sqlx::FromRow)]
struct AdminProjectRow {
    id: i64,
    name: String,
    collaborator_name: Option<String>,
}

#[derive(Clone, sqlx::FromRow)]
struct AdminRunRow {
    id: i64,
    name: String,
    project_id: i64,
    collaborator_name: Option<String>,
    status: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
}

#[derive(Clone, sqlx::FromRow)]
struct RunActivityRow {
    run_id: i64,
    last_record_at: Option<String>,
}

#[derive(Clone, sqlx::FromRow)]
struct ProjectCollaboratorRow {
    project_id: i64,
    collaborator_name: String,
}

#[derive(Serialize, Clone)]
struct AdminRunInfo {
    run_id: i64,
    run_name: String,
    collaborator_name: Option<String>,
    status: String,
    started_at: Option<String>,
    ended_at: Option<String>,
    last_record_at: Option<String>,
}

#[derive(Serialize, Clone)]
struct AdminProjectInfo {
    id: i64,
    name: String,
    default_collaborator: Option<String>,
    collaborators: Vec<String>,
    active_runs: Vec<AdminRunInfo>,
    latest_run: Option<AdminRunInfo>,
}


#[derive(Clone)]
struct AppState {
    db_pool: sqlx::SqlitePool,
       tx: broadcast::Sender<String>,
}

// --- REST API ---

async fn get_projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects: Vec<Project> = sqlx::query_as::<_, Project>("SELECT id, name FROM projects")
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]); // Graceful error handling

    match projects.len() {
        0 => (StatusCode::NOT_FOUND, "No projects found").into_response(),
        _ => (StatusCode::OK, Json(projects)).into_response(),
    }
}


async fn get_runs(Path(project_id): Path<i64>, State(state): State<AppState>) -> impl IntoResponse {
    let runs: Vec<Run> = sqlx::query_as::<_, Run>(
        "SELECT id, name, project_id, collaborator_name, status, started_at, ended_at FROM runs WHERE project_id = ? ORDER BY started_at DESC, id DESC",
    )
    .bind(project_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]); // Graceful error handling

    match runs.len() {
        0 => (StatusCode::NOT_FOUND, "No runs found for this project").into_response(),
        _ => (StatusCode::OK, Json(runs)).into_response(),
    }
}

async fn get_run(Path(run_id): Path<i64>, State(state): State<AppState>) -> impl IntoResponse {
    let run: Option<Run> = sqlx::query_as::<_, Run>(
        "SELECT id, name, project_id, collaborator_name, status, started_at, ended_at FROM runs WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap();

    match run {
        Some(run) => (StatusCode::OK, Json(run)).into_response(),
        None => (StatusCode::NOT_FOUND, "Run not found").into_response(),
    }
}


async fn get_record_points(Path(run_id): Path<i64>, State(state): State<AppState>) -> impl IntoResponse {
    let record_points: Vec<RecordPoint> = sqlx::query_as::<_, RecordPoint>(
        "SELECT id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy FROM records WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap(); // Graceful error handling

    match record_points.len() {
        0 => (StatusCode::NOT_FOUND, "No record points found for this run").into_response(),
        _ => (StatusCode::OK, Json(record_points)).into_response(),
    }
}

async fn get_run_summary(Path(run_id): Path<i64>, State(state): State<AppState>) -> impl IntoResponse {
    let summary: Option<RunSummary> = sqlx::query_as::<_, RunSummary>(
        "SELECT run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy FROM runsummary WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap(); // Graceful error handling

    match summary {
        Some(s) => (StatusCode::OK, Json(s)).into_response(),
        None => (StatusCode::NOT_FOUND, "Run not found").into_response(),
    }
}

async fn get_admin_projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects: Vec<AdminProjectRow> = sqlx::query_as::<_, AdminProjectRow>(
        "SELECT id, name, collaborator_name FROM projects ORDER BY name ASC",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    let runs: Vec<AdminRunRow> = sqlx::query_as::<_, AdminRunRow>(
        "SELECT id, name, project_id, collaborator_name, status, started_at, ended_at
         FROM runs
         ORDER BY project_id ASC, started_at DESC, id DESC",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    let activities: Vec<RunActivityRow> = sqlx::query_as::<_, RunActivityRow>(
        "SELECT run_id, MAX(timestamp) AS last_record_at FROM records GROUP BY run_id",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    let activity_by_run: HashMap<i64, Option<String>> = activities
        .into_iter()
        .map(|activity| (activity.run_id, activity.last_record_at))
        .collect();

    let collaborator_rows: Vec<ProjectCollaboratorRow> = sqlx::query_as::<_, ProjectCollaboratorRow>(
        "SELECT project_id, collaborator_name
         FROM project_collaborators
         ORDER BY project_id ASC, collaborator_name ASC",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    let mut collaborators_by_project: HashMap<i64, Vec<String>> = HashMap::new();
    for collaborator in collaborator_rows {
        collaborators_by_project
            .entry(collaborator.project_id)
            .or_default()
            .push(collaborator.collaborator_name);
    }

    let mut runs_by_project: HashMap<i64, Vec<AdminRunInfo>> = HashMap::new();
    for run in runs {
        runs_by_project
            .entry(run.project_id)
            .or_default()
            .push(AdminRunInfo {
                run_id: run.id,
                run_name: run.name,
                collaborator_name: run.collaborator_name,
                status: run.status.unwrap_or_else(|| "finished".to_string()),
                started_at: run.started_at,
                ended_at: run.ended_at,
                last_record_at: activity_by_run.get(&run.id).cloned().flatten(),
            });
    }

    let response: Vec<AdminProjectInfo> = projects
        .into_iter()
        .map(|project| {
            let project_runs = runs_by_project.remove(&project.id).unwrap_or_default();
            let latest_run = project_runs.first().cloned();
            let active_runs = project_runs
                .iter()
                .filter(|run| run.status == "running")
                .cloned()
                .collect();
            let collaborators = collaborators_by_project.remove(&project.id).unwrap_or_default();

            AdminProjectInfo {
                id: project.id,
                name: project.name,
                default_collaborator: project.collaborator_name,
                collaborators,
                active_runs,
                latest_run,
            }
        })
        .collect();

    (StatusCode::OK, Json(response)).into_response()
}

// --- SSE ---



// --- Routes ---
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the database connection pool
    let db_url = resolve_db_url();
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    ensure_schema(&db).await?;

    let (tx, _) = broadcast::channel::<String>(100);

    let state = AppState {
        db_pool: db.clone(),
        tx: tx.clone(),
    };
        // 3. Spawn the background watcher task
    tokio::spawn(watch_db_changes(db, tx));

    // Initialize the application state with the database connection pool
    let app = Router::new()
        .route("/api/hello", get(hello_world))
        .route("/api/projects", get(get_projects))
        .route("/api/project/{project_id}/runs", get(get_runs))
        .route("/api/run/{run_id}", get(get_run))
        .route("/api/run/{run_id}/record_points", get(get_record_points))   
        .route("/api/run/{run_id}/summary", get(get_run_summary))
        .route("/api/admin/projects", get(get_admin_projects))
        .route("/api/sse", get(sse_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Run the app with axum, listening globally on port 3001
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn resolve_db_url() -> String {
    if let Ok(db_url) = std::env::var("ECOCODE_DB_URL") {
        return db_url;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(|path| path.to_path_buf())
        .unwrap_or(manifest_dir);
    let db_path = repo_root.join("ecocode").join("ecocodeDB.db");

    format!("sqlite://{}", db_path.display())
}

async fn ensure_schema(db: &sqlx::SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query("PRAGMA journal_mode=WAL;").execute(db).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE,
            collaborator_name TEXT
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS project_collaborators (
            project_id INTEGER NOT NULL,
            collaborator_name TEXT NOT NULL,
            PRIMARY KEY (project_id, collaborator_name),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY,
            name TEXT,
            project_id INTEGER,
            collaborator_name TEXT,
            status TEXT NOT NULL DEFAULT 'finished',
            started_at TEXT,
            ended_at TEXT,
            FOREIGN KEY (project_id) REFERENCES projects(id)
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS records (
            id INTEGER PRIMARY KEY,
            run_id INTEGER,
            pid INTEGER,
            timestamp TEXT,
            cpu_usage REAL,
            cpu_energy REAL,
            gpu_usage REAL,
            gpu_energy REAL,
            mem_usage REAL,
            mem_energy REAL,
            igpu_usage REAL,
            igpu_energy REAL,
            FOREIGN KEY (run_id) REFERENCES runs(id)
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS runsummary (
            id INTEGER PRIMARY KEY,
            run_id INTEGER,
            total_cpu_energy REAL,
            total_gpu_energy REAL,
            total_mem_energy REAL,
            total_igpu_energy REAL,
            FOREIGN KEY (run_id) REFERENCES runs(id)
        )",
    )
    .execute(db)
    .await?;

    ensure_column(db, "projects", "collaborator_name", "ALTER TABLE projects ADD COLUMN collaborator_name TEXT").await?;
    ensure_column(db, "runs", "collaborator_name", "ALTER TABLE runs ADD COLUMN collaborator_name TEXT").await?;
    ensure_column(
        db,
        "runs",
        "status",
        "ALTER TABLE runs ADD COLUMN status TEXT NOT NULL DEFAULT 'finished'",
    )
    .await?;
    ensure_column(db, "runs", "started_at", "ALTER TABLE runs ADD COLUMN started_at TEXT").await?;
    ensure_column(db, "runs", "ended_at", "ALTER TABLE runs ADD COLUMN ended_at TEXT").await?;
    backfill_project_collaborators(db).await?;
    sync_project_shortcuts(db).await?;

    Ok(())
}

async fn ensure_column(
    db: &sqlx::SqlitePool,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if has_column(db, table, column).await? {
        return Ok(());
    }

    sqlx::query(alter_sql).execute(db).await?;
    Ok(())
}

async fn has_column(
    db: &sqlx::SqlitePool,
    table: &str,
    column: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(db)
        .await?;

    Ok(rows
        .iter()
        .any(|row| row.get::<String, _>("name") == column))
}

async fn backfill_project_collaborators(
    db: &sqlx::SqlitePool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "INSERT OR IGNORE INTO project_collaborators (project_id, collaborator_name)
         SELECT id, collaborator_name
         FROM projects
         WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO project_collaborators (project_id, collaborator_name)
         SELECT DISTINCT project_id, collaborator_name
         FROM runs
         WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''",
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn sync_project_shortcuts(
    db: &sqlx::SqlitePool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "UPDATE projects
         SET collaborator_name = (
             SELECT CASE
                 WHEN COUNT(*) = 1 THEN MAX(collaborator_name)
                 ELSE NULL
             END
             FROM project_collaborators
             WHERE project_id = projects.id
         )",
    )
    .execute(db)
    .await?;

    Ok(())
}


/// The SSE handler: Subscribes to the broadcast channel and streams messages to the client
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(msg) => Some(Ok(Event::default().data(msg))),
            Err(_) => None, // Ignore lagged/closed errors
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}



/// Background Task: Polls SQLite for version changes
async fn watch_db_changes(db: sqlx::SqlitePool, tx: broadcast::Sender<String>) {
    let mut last_version: i32 = 0;

    // Initialize version to current state
    if let Ok(row) = sqlx::query_as::<_, (i32,)>("PRAGMA user_version").fetch_one(&db).await {
        last_version = row.0;
    }

    loop {
        // Check the PRAGMA user_version (very cheap check)
        let res = sqlx::query_as::<_, (i32,)>("PRAGMA user_version")
            .fetch_one(&db)
            .await;

        if let Ok((current_version,)) = res {
            if current_version > last_version {
                // Something changed! Fetch the newest row
                let new_data: Result<RecordPoint, sqlx::Error>   = sqlx::query_as::<_, RecordPoint>("SELECT id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy FROM records ORDER BY id DESC LIMIT 1")
                    .fetch_one(&db)
                    .await;

                if let Ok(record) = new_data {
                    let _ = tx.send(format!("New Project: {}", serde_json::to_string(&record).unwrap_or_else(|_| "Failed to serialize".to_string())));
                }
                last_version = current_version;
            }
        }

        // Poll every 500ms to balance latency vs CPU usage
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}






// Simple "Hello, World!" endpoint
async fn hello_world() -> &'static str {
    "Hello, World!"
}
