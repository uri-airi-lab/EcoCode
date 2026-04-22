use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use dotenv::dotenv;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, SqlitePool, postgres::PgPoolOptions, sqlite::SqlitePoolOptions};
use std::{collections::HashMap, convert::Infallible, path::PathBuf};
use tokio::sync::broadcast;
use tokio::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn internal(err: impl std::fmt::Display) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum DataSource {
    Local,
    Online,
}

impl DataSource {
    fn parse(value: &str) -> Result<Self, ApiError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "online" => Ok(Self::Online),
            other => Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                format!("Unsupported source '{other}'. Expected 'local' or 'online'."),
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Online => "online",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct SourceQuery {
    source: Option<String>,
}

#[derive(Serialize)]
struct DataSourceResponse {
    default_source: DataSource,
    available_sources: Vec<DataSource>,
}

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
    command: Option<String>,
    cpu_name: Option<String>,
    cpu_cores: Option<i32>,
    ram_gb: Option<f64>,
    gpu_count: Option<i32>,
    gpu_names: Option<String>,
}

#[derive(Serialize, Clone, sqlx::FromRow)]
struct RecordPointRow {
    id: i64,
    run_id: i64,
    pid: i64,
    timestamp: String,
    cpu_usage: Option<f64>,
    cpu_energy: Option<f64>,
    gpu_usage: Option<f64>,
    gpu_energy: Option<f64>,
    mem_usage: Option<f64>,
    mem_energy: Option<f64>,
    igpu_usage: Option<f64>,
    igpu_energy: Option<f64>,
}

#[derive(Serialize, Clone)]
struct RecordPoint {
    id: i64,
    run_id: i64,
    pid: i64,
    timestamp: String,
    cpu_usage: Option<f64>,
    cpu_energy: Option<f64>,
    gpu_usage: Option<f64>,
    gpu_energy: Option<f64>,
    mem_usage: Option<f64>,
    mem_energy: Option<f64>,
    igpu_usage: Option<f64>,
    igpu_energy: Option<f64>,
    gpu_details: Vec<GpuRecordPoint>,
    socket_details: Vec<SocketRecordPoint>,
}

#[derive(Serialize, Clone, sqlx::FromRow)]
struct GpuRecordPoint {
    id: i64,
    record_id: i64,
    device_index: i32,
    gpu_usage: f64,
    gpu_energy: f64,
}

#[derive(Serialize, Clone, sqlx::FromRow)]
struct SocketRecordPoint {
    id: i64,
    record_id: i64,
    socket: i32,
    domain: String,
    energy: f64,
}

#[derive(Serialize, Clone, sqlx::FromRow, Debug)]
struct RunSummary {
    run_id: i64,
    total_cpu_energy: Option<f64>,
    total_gpu_energy: Option<f64>,
    total_mem_energy: Option<f64>,
    total_igpu_energy: Option<f64>,
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
    command: Option<String>,
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
    command: Option<String>,
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
    local_pool: SqlitePool,
    online_pool: Option<PgPool>,
    tx_local: broadcast::Sender<String>,
    tx_online: broadcast::Sender<String>,
    default_source: DataSource,
}

fn available_sources(state: &AppState) -> Vec<DataSource> {
    let mut sources = vec![DataSource::Local];
    if state.online_pool.is_some() {
        sources.push(DataSource::Online);
    }
    sources
}

fn resolve_source(state: &AppState, source_query: &SourceQuery) -> Result<DataSource, ApiError> {
    let requested = source_query
        .source
        .as_deref()
        .unwrap_or(state.default_source.as_str());

    let source = DataSource::parse(requested)?;

    if source == DataSource::Online && state.online_pool.is_none() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "The online data source is unavailable. Set DATABASE_URL for the backend.",
        ));
    }

    Ok(source)
}

fn resolve_default_source(has_online_pool: bool) -> DataSource {
    match std::env::var("ECOCODE_DATA_SOURCE") {
        Ok(value) => match DataSource::parse(&value) {
            Ok(DataSource::Online) if has_online_pool => DataSource::Online,
            Ok(DataSource::Online) => DataSource::Local,
            Ok(source) => source,
            Err(_) => DataSource::Local,
        },
        Err(_) => DataSource::Local,
    }
}

async fn get_data_sources(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(DataSourceResponse {
        default_source: state.default_source,
        available_sources: available_sources(&state),
    }))
}

async fn get_projects(
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let projects = fetch_projects(&state, source).await?;

    if projects.is_empty() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "No projects found"));
    }

    Ok((StatusCode::OK, Json(projects)))
}

async fn get_runs(
    Path(project_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let runs = fetch_runs(&state, source, project_id).await?;

    if runs.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No runs found for this project",
        ));
    }

    Ok((StatusCode::OK, Json(runs)))
}

async fn get_run(
    Path(run_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let run = fetch_run(&state, source, run_id).await?;

    match run {
        Some(run) => Ok((StatusCode::OK, Json(run)).into_response()),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found")),
    }
}

async fn get_record_points(
    Path(run_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let rows = fetch_record_rows(&state, source, run_id).await?;

    if rows.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "No record points found for this run",
        ));
    }

    let gpu_records = fetch_gpu_records_for_run(&state, source, run_id).await?;
    let socket_records = fetch_socket_records_for_run(&state, source, run_id).await?;

    let record_points = merge_record_points(rows, gpu_records, socket_records);
    Ok((StatusCode::OK, Json(record_points)))
}

async fn get_gpu_records(
    Path(run_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let gpu_records = fetch_gpu_records_for_run(&state, source, run_id).await?;
    Ok((StatusCode::OK, Json(gpu_records)))
}

async fn get_socket_records(
    Path(run_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let socket_records = fetch_socket_records_for_run(&state, source, run_id).await?;
    Ok((StatusCode::OK, Json(socket_records)))
}

async fn get_run_summary(
    Path(run_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let summary = fetch_run_summary(&state, source, run_id).await?;

    match summary {
        Some(summary) => Ok((StatusCode::OK, Json(summary)).into_response()),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "Run not found")),
    }
}

async fn get_admin_projects(
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let projects = fetch_admin_projects(&state, source).await?;
    Ok((StatusCode::OK, Json(projects)))
}

async fn sse_handler(
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let rx = match source {
        DataSource::Local => state.tx_local.subscribe(),
        DataSource::Online => state.tx_online.subscribe(),
    };

    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(msg) => Some(Ok(Event::default().data(msg))),
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn merge_record_points(
    rows: Vec<RecordPointRow>,
    gpu_records: Vec<GpuRecordPoint>,
    socket_records: Vec<SocketRecordPoint>,
) -> Vec<RecordPoint> {
    let mut gpu_by_record: HashMap<i64, Vec<GpuRecordPoint>> = HashMap::new();
    for record in gpu_records {
        gpu_by_record
            .entry(record.record_id)
            .or_default()
            .push(record);
    }

    let mut socket_by_record: HashMap<i64, Vec<SocketRecordPoint>> = HashMap::new();
    for record in socket_records {
        socket_by_record
            .entry(record.record_id)
            .or_default()
            .push(record);
    }

    rows.into_iter()
        .map(|row| RecordPoint {
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
            gpu_details: gpu_by_record.remove(&row.id).unwrap_or_default(),
            socket_details: socket_by_record.remove(&row.id).unwrap_or_default(),
        })
        .collect()
}

async fn fetch_projects(state: &AppState, source: DataSource) -> Result<Vec<Project>, ApiError> {
    match source {
        DataSource::Local => {
            sqlx::query_as::<_, Project>("SELECT id, name FROM projects ORDER BY name ASC")
                .fetch_all(&state.local_pool)
                .await
                .map_err(ApiError::internal)
        }
        DataSource::Online => {
            sqlx::query_as::<_, Project>("SELECT id, name FROM projects ORDER BY name ASC")
                .fetch_all(state.online_pool.as_ref().ok_or_else(|| {
                    ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
                })?)
                .await
                .map_err(ApiError::internal)
        }
    }
}

async fn fetch_runs(
    state: &AppState,
    source: DataSource,
    project_id: i64,
) -> Result<Vec<Run>, ApiError> {
    match source {
        DataSource::Local => sqlx::query_as::<_, Run>(
            "SELECT runs.id, runs.name, runs.project_id, runs.collaborator_name, runs.status, runs.started_at, runs.ended_at, runs.command, run_system_info.cpu_name, run_system_info.cpu_cores, run_system_info.ram_gb, run_system_info.gpu_count, run_system_info.gpu_names
             FROM runs
             LEFT JOIN run_system_info ON runs.id = run_system_info.run_id
             WHERE runs.project_id = ?
             ORDER BY runs.started_at DESC, runs.id DESC",
        )
        .bind(project_id)
        .fetch_all(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => {
            sqlx::query_as::<_, Run>(
            "SELECT runs.id, runs.name, runs.project_id, runs.collaborator_name, runs.status, runs.started_at, runs.ended_at, runs.command, run_system_info.cpu_name, run_system_info.cpu_cores, run_system_info.ram_gb, run_system_info.gpu_count, run_system_info.gpu_names
             FROM runs
             LEFT JOIN run_system_info ON runs.id = run_system_info.run_id
             WHERE runs.project_id = $1
             ORDER BY runs.started_at DESC, runs.id DESC",
            )
            .bind(project_id)
            .fetch_all(state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?)
            .await
            .map_err(ApiError::internal)
        }
    }
}

async fn fetch_run(
    state: &AppState,
    source: DataSource,
    run_id: i64,
) -> Result<Option<Run>, ApiError> {
    match source {
        DataSource::Local => sqlx::query_as::<_, Run>(
            "SELECT runs.id, runs.name, runs.project_id, runs.collaborator_name, runs.status, runs.started_at, runs.ended_at, runs.command, run_system_info.cpu_name, run_system_info.cpu_cores, run_system_info.ram_gb, run_system_info.gpu_count, run_system_info.gpu_names
             FROM runs
             LEFT JOIN run_system_info ON runs.id = run_system_info.run_id
             WHERE runs.id = ?",
        )
        .bind(run_id)
        .fetch_optional(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => {
            sqlx::query_as::<_, Run>(
                "SELECT runs.id, runs.name, runs.project_id, runs.collaborator_name, runs.status, runs.started_at, runs.ended_at, runs.command, run_system_info.cpu_name, run_system_info.cpu_cores, run_system_info.ram_gb, run_system_info.gpu_count, run_system_info.gpu_names
             FROM runs
             LEFT JOIN run_system_info ON runs.id = run_system_info.run_id
             WHERE runs.id = $1",
            )
            .bind(run_id)
            .fetch_optional(state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?)
            .await
            .map_err(ApiError::internal)
        }
    }
}

async fn fetch_record_rows(
    state: &AppState,
    source: DataSource,
    run_id: i64,
) -> Result<Vec<RecordPointRow>, ApiError> {
    match source {
        DataSource::Local => sqlx::query_as::<_, RecordPointRow>(
            "SELECT id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy,
                    mem_usage, mem_energy, igpu_usage, igpu_energy
             FROM records
             WHERE run_id = ?
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(run_id)
        .fetch_all(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => {
            sqlx::query_as::<_, RecordPointRow>(
                "SELECT id, run_id, pid, timestamp::text AS timestamp, cpu_usage, cpu_energy,
                    gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy
             FROM records
             WHERE run_id = $1
             ORDER BY timestamp ASC, id ASC",
            )
            .bind(run_id)
            .fetch_all(state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?)
            .await
            .map_err(ApiError::internal)
        }
    }
}

async fn fetch_gpu_records_for_run(
    state: &AppState,
    source: DataSource,
    run_id: i64,
) -> Result<Vec<GpuRecordPoint>, ApiError> {
    match source {
        DataSource::Local => sqlx::query_as::<_, GpuRecordPoint>(
            "SELECT gr.id, gr.record_id, gr.device_index, gr.gpu_usage, gr.gpu_energy
             FROM gpu_records gr
             INNER JOIN records r ON r.id = gr.record_id
             WHERE r.run_id = ?
             ORDER BY gr.record_id ASC, gr.device_index ASC",
        )
        .bind(run_id)
        .fetch_all(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => {
            sqlx::query_as::<_, GpuRecordPoint>(
                "SELECT gr.id, gr.record_id, gr.device_index, gr.gpu_usage, gr.gpu_energy
             FROM gpu_records gr
             INNER JOIN records r ON r.id = gr.record_id
             WHERE r.run_id = $1
             ORDER BY gr.record_id ASC, gr.device_index ASC",
            )
            .bind(run_id)
            .fetch_all(state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?)
            .await
            .map_err(ApiError::internal)
        }
    }
}

async fn fetch_socket_records_for_run(
    state: &AppState,
    source: DataSource,
    run_id: i64,
) -> Result<Vec<SocketRecordPoint>, ApiError> {
    match source {
        DataSource::Local => sqlx::query_as::<_, SocketRecordPoint>(
            "SELECT sr.id, sr.record_id, sr.socket, sr.domain, sr.energy
             FROM socket_records sr
             INNER JOIN records r ON r.id = sr.record_id
             WHERE r.run_id = ?
             ORDER BY sr.record_id ASC, sr.socket ASC, sr.domain ASC",
        )
        .bind(run_id)
        .fetch_all(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => {
            sqlx::query_as::<_, SocketRecordPoint>(
                "SELECT sr.id, sr.record_id, sr.socket, sr.domain, sr.energy
             FROM socket_records sr
             INNER JOIN records r ON r.id = sr.record_id
             WHERE r.run_id = $1
             ORDER BY sr.record_id ASC, sr.socket ASC, sr.domain ASC",
            )
            .bind(run_id)
            .fetch_all(state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?)
            .await
            .map_err(ApiError::internal)
        }
    }
}

async fn fetch_run_summary(
    state: &AppState,
    source: DataSource,
    run_id: i64,
) -> Result<Option<RunSummary>, ApiError> {
    match source {
        DataSource::Local => sqlx::query_as::<_, RunSummary>(
            "SELECT run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy
             FROM runsummary
             WHERE run_id = ?",
        )
        .bind(run_id)
        .fetch_optional(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => sqlx::query_as::<_, RunSummary>(
            "SELECT run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy
             FROM runsummary
             WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_optional(
            state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?,
        )
        .await
        .map_err(ApiError::internal),
    }
}

async fn fetch_admin_projects(
    state: &AppState,
    source: DataSource,
) -> Result<Vec<AdminProjectInfo>, ApiError> {
    let (projects, runs, activities, collaborator_rows) = match source {
        DataSource::Local => {
            let projects = sqlx::query_as::<_, AdminProjectRow>(
                "SELECT id, name, collaborator_name FROM projects ORDER BY name ASC",
            )
            .fetch_all(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;

            let runs = sqlx::query_as::<_, AdminRunRow>(
                "SELECT id, name, project_id, collaborator_name, status, started_at, ended_at, command
                 FROM runs
                 ORDER BY project_id ASC, started_at DESC, id DESC",
            )
            .fetch_all(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;

            let activities = sqlx::query_as::<_, RunActivityRow>(
                "SELECT run_id, MAX(timestamp) AS last_record_at
                 FROM records
                 GROUP BY run_id",
            )
            .fetch_all(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;

            let collaborator_rows = sqlx::query_as::<_, ProjectCollaboratorRow>(
                "SELECT project_id, collaborator_name
                 FROM project_collaborators
                 ORDER BY project_id ASC, collaborator_name ASC",
            )
            .fetch_all(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;

            (projects, runs, activities, collaborator_rows)
        }
        DataSource::Online => {
            let pool = state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?;

            let projects = sqlx::query_as::<_, AdminProjectRow>(
                "SELECT id, name, collaborator_name FROM projects ORDER BY name ASC",
            )
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;

            let runs = sqlx::query_as::<_, AdminRunRow>(
                "SELECT id, name, project_id, collaborator_name, status, started_at, ended_at, command
                 FROM runs
                 ORDER BY project_id ASC, started_at DESC, id DESC",
            )
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;

            let activities = sqlx::query_as::<_, RunActivityRow>(
                "SELECT run_id, MAX(timestamp)::text AS last_record_at
                 FROM records
                 GROUP BY run_id",
            )
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;

            let collaborator_rows = sqlx::query_as::<_, ProjectCollaboratorRow>(
                "SELECT project_id, collaborator_name
                 FROM project_collaborators
                 ORDER BY project_id ASC, collaborator_name ASC",
            )
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;

            (projects, runs, activities, collaborator_rows)
        }
    };

    let activity_by_run: HashMap<i64, Option<String>> = activities
        .into_iter()
        .map(|activity| (activity.run_id, activity.last_record_at))
        .collect();

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
                command: run.command,
            });
    }

    Ok(projects
        .into_iter()
        .map(|project| {
            let project_runs = runs_by_project.remove(&project.id).unwrap_or_default();
            let latest_run = project_runs.first().cloned();
            let active_runs = project_runs
                .iter()
                .filter(|run| run.status == "running")
                .cloned()
                .collect();
            let collaborators = collaborators_by_project
                .remove(&project.id)
                .unwrap_or_default();

            AdminProjectInfo {
                id: project.id,
                name: project.name,
                default_collaborator: project.collaborator_name,
                collaborators,
                active_runs,
                latest_run,
            }
        })
        .collect())
}

async fn fetch_latest_record_with_details(
    state: &AppState,
    source: DataSource,
) -> Result<Option<RecordPoint>, ApiError> {
    let latest_row =
        match source {
            DataSource::Local => sqlx::query_as::<_, RecordPointRow>(
                "SELECT id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy,
                    mem_usage, mem_energy, igpu_usage, igpu_energy
             FROM records
             ORDER BY id DESC
             LIMIT 1",
            )
            .fetch_optional(&state.local_pool)
            .await
            .map_err(ApiError::internal)?,
            DataSource::Online => sqlx::query_as::<_, RecordPointRow>(
                "SELECT id, run_id, pid, timestamp::text AS timestamp, cpu_usage, cpu_energy,
                    gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy
             FROM records
             ORDER BY id DESC
             LIMIT 1",
            )
            .fetch_optional(state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?)
            .await
            .map_err(ApiError::internal)?,
        };

    let Some(row) = latest_row else {
        return Ok(None);
    };

    let (gpu_details, socket_details) = match source {
        DataSource::Local => {
            let gpu_details = sqlx::query_as::<_, GpuRecordPoint>(
                "SELECT id, record_id, device_index, gpu_usage, gpu_energy
                 FROM gpu_records
                 WHERE record_id = ?",
            )
            .bind(row.id)
            .fetch_all(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;

            let socket_details = sqlx::query_as::<_, SocketRecordPoint>(
                "SELECT id, record_id, socket, domain, energy
                 FROM socket_records
                 WHERE record_id = ?",
            )
            .bind(row.id)
            .fetch_all(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;

            (gpu_details, socket_details)
        }
        DataSource::Online => {
            let pool = state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?;

            let gpu_details = sqlx::query_as::<_, GpuRecordPoint>(
                "SELECT id, record_id, device_index, gpu_usage, gpu_energy
                 FROM gpu_records
                 WHERE record_id = $1",
            )
            .bind(row.id)
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;

            let socket_details = sqlx::query_as::<_, SocketRecordPoint>(
                "SELECT id, record_id, socket, domain, energy
                 FROM socket_records
                 WHERE record_id = $1",
            )
            .bind(row.id)
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;

            (gpu_details, socket_details)
        }
    };

    Ok(Some(RecordPoint {
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
    }))
}

async fn watch_source_changes(state: AppState, source: DataSource) {
    let tx = match source {
        DataSource::Local => state.tx_local.clone(),
        DataSource::Online => state.tx_online.clone(),
    };
    let mut last_record_id = 0_i64;

    loop {
        match fetch_latest_record_with_details(&state, source).await {
            Ok(Some(record)) if record.id > last_record_id => {
                if let Ok(payload) = serde_json::to_string(&record) {
                    let _ = tx.send(payload);
                }
                last_record_id = record.id;
            }
            Ok(Some(record)) => {
                last_record_id = last_record_id.max(record.id);
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!(
                    "watcher error for {} source: {}",
                    source.as_str(),
                    err.message
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn resolve_local_db_url() -> String {
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

fn resolve_online_db_url() -> Option<String> {
    std::env::var("ECOCODE_ONLINE_DB_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn ensure_sqlite_schema(db: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
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

    ensure_sqlite_column(
        db,
        "projects",
        "collaborator_name",
        "ALTER TABLE projects ADD COLUMN collaborator_name TEXT",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runs",
        "collaborator_name",
        "ALTER TABLE runs ADD COLUMN collaborator_name TEXT",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runs",
        "status",
        "ALTER TABLE runs ADD COLUMN status TEXT NOT NULL DEFAULT 'finished'",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runs",
        "started_at",
        "ALTER TABLE runs ADD COLUMN started_at TEXT",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runs",
        "ended_at",
        "ALTER TABLE runs ADD COLUMN ended_at TEXT",
    )
    .await?;
    backfill_sqlite_project_collaborators(db).await?;
    sync_sqlite_project_shortcuts(db).await?;

    Ok(())
}

async fn ensure_sqlite_column(
    db: &SqlitePool,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if has_sqlite_column(db, table, column).await? {
        return Ok(());
    }

    sqlx::query(alter_sql).execute(db).await?;
    Ok(())
}

async fn has_sqlite_column(
    db: &SqlitePool,
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

async fn backfill_sqlite_project_collaborators(
    db: &SqlitePool,
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

async fn sync_sqlite_project_shortcuts(db: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
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

async fn ensure_postgres_schema(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id BIGSERIAL PRIMARY KEY,
            name TEXT UNIQUE,
            collaborator_name TEXT
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS project_collaborators (
            project_id BIGINT NOT NULL,
            collaborator_name TEXT NOT NULL,
            PRIMARY KEY (project_id, collaborator_name),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS runs (
            id BIGSERIAL PRIMARY KEY,
            name TEXT,
            project_id BIGINT REFERENCES projects(id),
            collaborator_name TEXT,
            status TEXT NOT NULL DEFAULT 'finished',
            started_at TEXT,
            ended_at TEXT
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS records (
            id BIGSERIAL PRIMARY KEY,
            run_id BIGINT REFERENCES runs(id),
            pid BIGINT,
            timestamp TIMESTAMPTZ,
            cpu_usage DOUBLE PRECISION,
            cpu_energy DOUBLE PRECISION,
            gpu_usage DOUBLE PRECISION,
            gpu_energy DOUBLE PRECISION,
            mem_usage DOUBLE PRECISION,
            mem_energy DOUBLE PRECISION,
            igpu_usage DOUBLE PRECISION,
            igpu_energy DOUBLE PRECISION
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS gpu_records (
            id BIGSERIAL PRIMARY KEY,
            record_id BIGINT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
            device_index INTEGER NOT NULL,
            gpu_usage DOUBLE PRECISION,
            gpu_energy DOUBLE PRECISION
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS socket_records (
            id BIGSERIAL PRIMARY KEY,
            record_id BIGINT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
            socket INTEGER NOT NULL,
            domain TEXT NOT NULL,
            energy DOUBLE PRECISION
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS runsummary (
            id BIGSERIAL PRIMARY KEY,
            run_id BIGINT REFERENCES runs(id),
            total_cpu_energy DOUBLE PRECISION,
            total_gpu_energy DOUBLE PRECISION,
            total_mem_energy DOUBLE PRECISION,
            total_igpu_energy DOUBLE PRECISION
        )",
    )
    .execute(db)
    .await?;

    ensure_postgres_column(
        db,
        "projects",
        "collaborator_name",
        "ALTER TABLE projects ADD COLUMN collaborator_name TEXT",
    )
    .await?;

    ensure_postgres_column(
        db,
        "runs",
        "collaborator_name",
        "ALTER TABLE runs ADD COLUMN collaborator_name TEXT",
    )
    .await?;

    ensure_postgres_column(
        db,
        "runs",
        "status",
        "ALTER TABLE runs ADD COLUMN status TEXT NOT NULL DEFAULT 'finished'",
    )
    .await?;

    ensure_postgres_column(
        db,
        "runs",
        "started_at",
        "ALTER TABLE runs ADD COLUMN started_at TEXT",
    )
    .await?;

    ensure_postgres_column(
        db,
        "runs",
        "ended_at",
        "ALTER TABLE runs ADD COLUMN ended_at TEXT",
    )
    .await?;

    backfill_postgres_project_collaborators(db).await?;
    sync_postgres_project_shortcuts(db).await?;

    Ok(())
}

async fn ensure_postgres_column(
    db: &PgPool,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if has_postgres_column(db, table, column).await? {
        return Ok(());
    }

    sqlx::query(alter_sql).execute(db).await?;
    Ok(())
}

async fn has_postgres_column(
    db: &PgPool,
    table: &str,
    column: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let row = sqlx::query(
        "SELECT EXISTS (
            SELECT 1 
            FROM information_schema.columns 
            WHERE table_name = $1 AND column_name = $2
        )",
    )
    .bind(table)
    .bind(column)
    .fetch_one(db)
    .await?;

    Ok(row.get(0))
}

async fn backfill_postgres_project_collaborators(
    db: &PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "INSERT INTO project_collaborators (project_id, collaborator_name)
         SELECT id, collaborator_name
         FROM projects
         WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''
         ON CONFLICT DO NOTHING",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO project_collaborators (project_id, collaborator_name)
         SELECT DISTINCT project_id, collaborator_name
         FROM runs
         WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''
         ON CONFLICT DO NOTHING",
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn sync_postgres_project_shortcuts(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
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

async fn hello_world() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let local_db_url = resolve_local_db_url();
    let local_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&local_db_url)
        .await?;
    println!("Local SQLite pool initialized at {}", local_db_url);
    ensure_sqlite_schema(&local_pool).await?;

    let online_pool = match resolve_online_db_url() {
        Some(db_url) => {
            println!("Initializing online PostgreSQL pool...");
            let pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(&db_url)
                .await?;
            ensure_postgres_schema(&pool).await?;
            println!("Online PostgreSQL pool initialized.");
            Some(pool)
        }
        None => {
            println!("No online database configuration found (ECOCODE_ONLINE_DB_URL or DATABASE_URL missing).");
            None
        }
    };

    let default_source = resolve_default_source(online_pool.is_some());
    let (tx_local, _) = broadcast::channel::<String>(100);
    let (tx_online, _) = broadcast::channel::<String>(100);

    let state = AppState {
        local_pool: local_pool.clone(),
        online_pool: online_pool.clone(),
        tx_local: tx_local.clone(),
        tx_online: tx_online.clone(),
        default_source,
    };

    tokio::spawn(watch_source_changes(state.clone(), DataSource::Local));
    if online_pool.is_some() {
        tokio::spawn(watch_source_changes(state.clone(), DataSource::Online));
    }

    let app = Router::new()
        .route("/api/hello", get(hello_world))
        .route("/api/data-sources", get(get_data_sources))
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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
