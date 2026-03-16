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
struct RecordPointRow {
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

#[derive(Serialize, Clone)]
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
    gpu_details: Vec<GpuRecordPoint>,
    socket_details: Vec<SocketRecordPoint>,
}

#[derive(Serialize, Clone, sqlx::FromRow)]
struct GpuRecordPoint {
    id: i64,
    record_id: i64,
    device_index: i64,
    gpu_usage: f64,
    gpu_energy: f64,
}

#[derive(Serialize, Clone, sqlx::FromRow)]
struct SocketRecordPoint {
    id: i64,
    record_id: i64,
    socket: i64,
    domain: String,
    energy: f64,
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
    let rows: Vec<RecordPointRow> = sqlx::query_as::<_, RecordPointRow>(
        "SELECT id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy FROM records WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    if rows.is_empty() {
        return (StatusCode::NOT_FOUND, "No record points found for this run").into_response();
    }

    let record_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();

    // Fetch per-GPU records for all records in one query
    let placeholders: String = record_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let gpu_query = format!(
        "SELECT id, record_id, device_index, gpu_usage, gpu_energy FROM gpu_records WHERE record_id IN ({})",
        placeholders
    );
    let mut gpu_q = sqlx::query_as::<_, GpuRecordPoint>(&gpu_query);
    for id in &record_ids {
        gpu_q = gpu_q.bind(*id);
    }
    let gpu_records: Vec<GpuRecordPoint> = gpu_q.fetch_all(&state.db_pool).await.unwrap_or_else(|_| vec![]);

    // Fetch per-socket records
    let socket_query = format!(
        "SELECT id, record_id, socket, domain, energy FROM socket_records WHERE record_id IN ({})",
        placeholders
    );
    let mut socket_q = sqlx::query_as::<_, SocketRecordPoint>(&socket_query);
    for id in &record_ids {
        socket_q = socket_q.bind(*id);
    }
    let socket_records: Vec<SocketRecordPoint> = socket_q.fetch_all(&state.db_pool).await.unwrap_or_else(|_| vec![]);

    // Group by record_id
    let mut gpu_by_record: std::collections::HashMap<i64, Vec<GpuRecordPoint>> = std::collections::HashMap::new();
    for gr in gpu_records {
        gpu_by_record.entry(gr.record_id).or_default().push(gr);
    }
    let mut socket_by_record: std::collections::HashMap<i64, Vec<SocketRecordPoint>> = std::collections::HashMap::new();
    for sr in socket_records {
        socket_by_record.entry(sr.record_id).or_default().push(sr);
    }

    let record_points: Vec<RecordPoint> = rows.into_iter().map(|r| {
        RecordPoint {
            id: r.id,
            run_id: r.run_id,
            pid: r.pid,
            timestamp: r.timestamp,
            cpu_usage: r.cpu_usage,
            cpu_energy: r.cpu_energy,
            gpu_usage: r.gpu_usage,
            gpu_energy: r.gpu_energy,
            mem_usage: r.mem_usage,
            mem_energy: r.mem_energy,
            igpu_usage: r.igpu_usage,
            igpu_energy: r.igpu_energy,
            gpu_details: gpu_by_record.remove(&r.id).unwrap_or_default(),
            socket_details: socket_by_record.remove(&r.id).unwrap_or_default(),
        }
    }).collect();

    (StatusCode::OK, Json(record_points)).into_response()
}

async fn get_gpu_records(Path(run_id): Path<i64>, State(state): State<AppState>) -> impl IntoResponse {
    let gpu_records: Vec<GpuRecordPoint> = sqlx::query_as::<_, GpuRecordPoint>(
        "SELECT gr.id, gr.record_id, gr.device_index, gr.gpu_usage, gr.gpu_energy
         FROM gpu_records gr
         INNER JOIN records r ON r.id = gr.record_id
         WHERE r.run_id = ?
         ORDER BY gr.record_id ASC, gr.device_index ASC",
    )
    .bind(run_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    (StatusCode::OK, Json(gpu_records)).into_response()
}

async fn get_socket_records(Path(run_id): Path<i64>, State(state): State<AppState>) -> impl IntoResponse {
    let socket_records: Vec<SocketRecordPoint> = sqlx::query_as::<_, SocketRecordPoint>(
        "SELECT sr.id, sr.record_id, sr.socket, sr.domain, sr.energy
         FROM socket_records sr
         INNER JOIN records r ON r.id = sr.record_id
         WHERE r.run_id = ?
         ORDER BY sr.record_id ASC, sr.socket ASC, sr.domain ASC",
    )
    .bind(run_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_else(|_| vec![]);

    (StatusCode::OK, Json(socket_records)).into_response()
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
        .route("/api/run/{run_id}/gpu_records", get(get_gpu_records))
        .route("/api/run/{run_id}/socket_records", get(get_socket_records))
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
        "CREATE TABLE IF NOT EXISTS gpu_records (
            id INTEGER PRIMARY KEY,
            record_id INTEGER NOT NULL,
            device_index INTEGER NOT NULL,
            gpu_usage REAL,
            gpu_energy REAL,
            FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS socket_records (
            id INTEGER PRIMARY KEY,
            record_id INTEGER NOT NULL,
            socket INTEGER NOT NULL,
            domain TEXT NOT NULL,
            energy REAL,
            FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
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
                let new_data: Result<RecordPointRow, sqlx::Error> = sqlx::query_as::<_, RecordPointRow>("SELECT id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy FROM records ORDER BY id DESC LIMIT 1")
                    .fetch_one(&db)
                    .await;

                if let Ok(row) = new_data {
                    // Fetch per-GPU and per-socket details for this record
                    let gpu_details: Vec<GpuRecordPoint> = sqlx::query_as::<_, GpuRecordPoint>(
                        "SELECT id, record_id, device_index, gpu_usage, gpu_energy FROM gpu_records WHERE record_id = ?"
                    )
                    .bind(row.id)
                    .fetch_all(&db)
                    .await
                    .unwrap_or_else(|_| vec![]);

                    let socket_details: Vec<SocketRecordPoint> = sqlx::query_as::<_, SocketRecordPoint>(
                        "SELECT id, record_id, socket, domain, energy FROM socket_records WHERE record_id = ?"
                    )
                    .bind(row.id)
                    .fetch_all(&db)
                    .await
                    .unwrap_or_else(|_| vec![]);

                    let record = RecordPoint {
                        id: row.id,
                        run_id: row.run_id,
                        pid: row.pid,
                        timestamp: row.timestamp,
                        cpu_usage: row.cpu_usage,
                        cpu_energy: row.cpu_energy,
                        gpu_usage: row.gpu_usage,
                        gpu_energy: row.gpu_energy,
                        mem_usage: row.mem_usage,
                        mem_energy: row.mem_energy,
                        igpu_usage: row.igpu_usage,
                        igpu_energy: row.igpu_energy,
                        gpu_details,
                        socket_details,
                    };

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
