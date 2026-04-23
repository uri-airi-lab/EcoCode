use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
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

/// Static carbon intensity lookup table (gCO₂eq/kWh).
const CARBON_REGIONS: &[(&str, &str, f64)] = &[
    ("WORLD", "Global Average", 494.0),
    ("FR", "France", 52.0),
    ("SE", "Sweden", 13.0),
    ("NO", "Norway", 28.0),
    ("IS", "Iceland", 28.0),
    ("CH", "Switzerland", 43.0),
    ("AT", "Austria", 158.0),
    ("GB", "United Kingdom", 207.0),
    ("IE", "Ireland", 295.0),
    ("PT", "Portugal", 167.0),
    ("ES", "Spain", 163.0),
    ("IT", "Italy", 233.0),
    ("NL", "Netherlands", 295.0),
    ("BE", "Belgium", 167.0),
    ("DK", "Denmark", 156.0),
    ("FI", "Finland", 76.0),
    ("DE", "Germany", 380.0),
    ("PL", "Poland", 635.0),
    ("CZ", "Czech Republic", 425.0),
    ("RO", "Romania", 280.0),
    ("UA", "Ukraine", 296.0),
    ("TR", "Turkey", 400.0),
    ("RU", "Russia", 340.0),
    ("US", "United States", 367.0),
    ("CA", "Canada", 130.0),
    ("MX", "Mexico", 415.0),
    ("BR", "Brazil", 91.0),
    ("AR", "Argentina", 263.0),
    ("CL", "Chile", 290.0),
    ("CO", "Colombia", 175.0),
    ("VE", "Venezuela", 160.0),
    ("ZA", "South Africa", 700.0),
    ("NG", "Nigeria", 430.0),
    ("EG", "Egypt", 450.0),
    ("MA", "Morocco", 540.0),
    ("DZ", "Algeria", 490.0),
    ("AE", "United Arab Emirates", 450.0),
    ("SA", "Saudi Arabia", 530.0),
    ("IL", "Israel", 480.0),
    ("IN", "India", 680.0),
    ("PK", "Pakistan", 430.0),
    ("BD", "Bangladesh", 545.0),
    ("CN", "China", 555.0),
    ("JP", "Japan", 471.0),
    ("KR", "South Korea", 415.0),
    ("TW", "Taiwan", 480.0),
    ("TH", "Thailand", 480.0),
    ("VN", "Vietnam", 360.0),
    ("ID", "Indonesia", 660.0),
    ("MY", "Malaysia", 585.0),
    ("SG", "Singapore", 430.0),
    ("PH", "Philippines", 580.0),
    ("AU", "Australia", 510.0),
    ("NZ", "New Zealand", 147.0),
];

fn static_intensity(country_code: &str) -> f64 {
    let code = country_code.trim().to_uppercase();
    CARBON_REGIONS
        .iter()
        .find(|(c, _, _)| *c == code.as_str())
        .map(|(_, _, v)| *v)
        .unwrap_or(494.0)
}




// TODO: Break down the file into smaller files


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

#[derive(Deserialize)]
struct CreateProjectPayload {
    name: String,
    intensity: Option<f64>,
    cost: Option<f64>,
    currency: Option<String>,
    address: Option<String>,
    use_api_for_cost: Option<bool>,
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
    total_co2_g: Option<f64>,
    carbon_intensity_used: Option<f64>,
    pue_used: Option<f64>,
    total_cost: Option<f64>,
    currency_used: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ElectricityMapsPriceResponse {
    pub price: f64,
    pub currency: String,
}

#[derive(Serialize, Clone, sqlx::FromRow, Debug)]
struct CarbonConfigRow {
    country_code: String,
    address: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    electricity_maps_api_key: Option<String>,
    resolved_intensity_g_per_kwh: f64,
    intensity_source: String,
    intensity_fetched_at: Option<String>,
    pue_factor: f64,
    manual_cost_per_kwh: Option<f64>,
    manual_currency: Option<String>,
    use_api_for_cost: bool,
    updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
struct CarbonConfigResponse {
    country_code: String,
    address: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    has_api_key: bool,
    resolved_intensity_g_per_kwh: f64,
    intensity_source: String,
    intensity_fetched_at: Option<String>,
    pue_factor: f64,
    manual_cost_per_kwh: Option<f64>,
    manual_currency: Option<String>,
    use_api_for_cost: bool,
    updated_at: String,
}

#[derive(Deserialize, Debug)]
struct CarbonConfigUpdate {
    address: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    country_code: Option<String>,
    electricity_maps_api_key: Option<String>,
    #[serde(default)]
    clear_api_key: bool,
    pue_factor: Option<f64>,
    manual_cost_per_kwh: Option<f64>,
    manual_currency: Option<String>,
    use_api_for_cost: Option<bool>,
}

#[derive(Serialize, Clone, Debug)]
struct CarbonRegion {
    code: String,
    name: String,
    intensity_g_per_kwh: f64,
}

#[derive(Deserialize, Debug)]
struct NominatimResult {
    lat: String,
    lon: String,
    display_name: String,
}

#[derive(Deserialize, Debug)]
struct ElectricityMapsResponse {
    #[serde(rename = "carbonIntensity")]
    carbon_intensity: f64,
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

async fn post_projects(
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
    Json(payload): Json<CreateProjectPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    
    let now = chrono::Utc::now().to_rfc3339();
    
    // Step 1: Resolve intensity and location
    let mut lat = None;
    let mut lon = None;
    let mut resolved_address = payload.address.clone();
    let intensity = payload.intensity.unwrap_or(494.0);
    let intensity_src = if payload.intensity.is_some() { "manual".to_string() } else { "static_WORLD".to_string() };

    if let Some(addr) = &payload.address {
        if let Ok((l_lat, l_lon, display)) = geocode_address(addr).await {
            lat = Some(l_lat);
            lon = Some(l_lon);
            resolved_address = Some(display);
            
            // If no manual intensity, try to fetch from static regions at least
            if payload.intensity.is_none() {
                // We don't have country code here easily, so we stay with WORLD or 
                // we could try to resolve it from geocoding.
                // For now, keep it simple.
            }
        }
    }

    let cost = payload.cost;
    let currency = payload.currency.unwrap_or_else(|| "USD".to_string());
    let use_api_for_cost = payload.use_api_for_cost.unwrap_or(false);

    let (id, name) = match source {
        DataSource::Local => {
            let result = sqlx::query(
                "INSERT INTO projects (name, resolved_intensity_g_per_kwh, intensity_source, manual_cost_per_kwh, manual_currency, address, latitude, longitude, use_api_for_cost, updated_at) 
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) 
                 RETURNING id, name"
            )
            .bind(&payload.name)
            .bind(intensity)
            .bind(intensity_src)
            .bind(cost)
            .bind(&currency)
            .bind(resolved_address)
            .bind(lat)
            .bind(lon)
            .bind(use_api_for_cost)
            .bind(&now)
            .fetch_one(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;
            
            (result.get::<i64, _>("id"), result.get::<String, _>("name"))
        }
        DataSource::Online => {
            let pool = state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?;
            let result = sqlx::query(
                "INSERT INTO projects (name, resolved_intensity_g_per_kwh, intensity_source, manual_cost_per_kwh, manual_currency, address, latitude, longitude, use_api_for_cost, updated_at) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) 
                 RETURNING id, name"
            )
            .bind(&payload.name)
            .bind(intensity)
            .bind(intensity_src)
            .bind(cost)
            .bind(&currency)
            .bind(resolved_address)
            .bind(lat)
            .bind(lon)
            .bind(use_api_for_cost)
            .bind(&now)
            .fetch_one(pool)
            .await
            .map_err(ApiError::internal)?;
            
            (result.get::<i64, _>("id"), result.get::<String, _>("name"))
        }
    };

    Ok((StatusCode::CREATED, Json(Project { id, name })))
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
            "SELECT run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy,
                    total_co2_g, carbon_intensity_used, pue_used, total_cost, currency_used
             FROM runsummary
             WHERE run_id = ?",
        )
        .bind(run_id)
        .fetch_optional(&state.local_pool)
        .await
        .map_err(ApiError::internal),
        DataSource::Online => sqlx::query_as::<_, RunSummary>(
            "SELECT run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy,
                    total_co2_g, carbon_intensity_used, pue_used, total_cost, currency_used
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

// --- Carbon API helpers ---

async fn geocode_address(address: &str) -> Result<(f64, f64, String), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .user_agent("EcoCode/1.0 (carbon emission tracker)")
        .build()?;
    let results: Vec<NominatimResult> = client
        .get("https://nominatim.openstreetmap.org/search")
        .query(&[("q", address), ("format", "json"), ("limit", "1")])
        .send()
        .await?
        .json()
        .await?;
    let first = results.into_iter().next().ok_or("No geocoding results found")?;
    let lat: f64 = first.lat.parse()?;
    let lon: f64 = first.lon.parse()?;
    println!("Geocoding successful: {} -> ({}, {})", address, lat, lon);
    Ok((lat, lon, first.display_name))
}

async fn fetch_electricity_maps_intensity(lat: f64, lon: f64, api_key: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.electricitymap.org/v3/carbon-intensity/latest?lat={}&lon={}",
        lat, lon
    );
    let res = client
        .get(&url)
        .header("auth-token", api_key)
        .send()
        .await?;
    if !res.status().is_success() {
        let msg = res.text().await?;
        println!("Electricity Maps Intensity API error: {}", msg);
        return Err(format!("Intensity API error: {}", msg).into());
    }
    let data: ElectricityMapsResponse = res.json().await?;
    println!("Fetched real-time intensity: {} gCO2/kWh", data.carbon_intensity);
    Ok(data.carbon_intensity)
}

async fn fetch_electricity_maps_price(lat: f64, lon: f64, api_key: &str) -> Result<(f64, String), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.electricitymap.org/v3/power-price/latest?lat={}&lon={}",
        lat, lon
    );
    let res = client
        .get(&url)
        .header("auth-token", api_key)
        .send()
        .await?;
    if !res.status().is_success() {
        let msg = res.text().await?;
        println!("Electricity Maps Price API error: {}", msg);
        return Err(format!("Price API error: {}", msg).into());
    }
    let data: ElectricityMapsPriceResponse = res.json().await?;
    println!("Fetched real-time price: {} {}", data.price, data.currency);
    Ok((data.price, data.currency))
}

fn make_carbon_config_response(row: CarbonConfigRow) -> CarbonConfigResponse {
    CarbonConfigResponse {
        has_api_key: row.electricity_maps_api_key.as_deref().map_or(false, |k| !k.is_empty()),
        country_code: row.country_code,
        address: row.address,
        latitude: row.latitude,
        longitude: row.longitude,
        resolved_intensity_g_per_kwh: row.resolved_intensity_g_per_kwh,
        intensity_source: row.intensity_source,
        intensity_fetched_at: row.intensity_fetched_at,
        pue_factor: row.pue_factor,
        manual_cost_per_kwh: row.manual_cost_per_kwh,
        manual_currency: row.manual_currency,
        use_api_for_cost: row.use_api_for_cost,
        updated_at: row.updated_at,
    }
}

async fn get_project_carbon_config(
    state: &AppState,
    source: DataSource,
    project_id: i64,
) -> Result<CarbonConfigRow, ApiError> {
    match source {
        DataSource::Local => {
            sqlx::query_as::<_, CarbonConfigRow>(
                "SELECT country_code, address, latitude, longitude, electricity_maps_api_key,
                        resolved_intensity_g_per_kwh, intensity_source, intensity_fetched_at,
                        pue_factor, manual_cost_per_kwh, manual_currency, use_api_for_cost, updated_at
                 FROM projects WHERE id = ?",
            )
            .bind(project_id)
            .fetch_optional(&state.local_pool)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, format!("Project {project_id} not found in local DB")))
        }
        DataSource::Online => {
            let pool = state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?;
            sqlx::query_as::<_, CarbonConfigRow>(
                "SELECT country_code, address, latitude, longitude, electricity_maps_api_key,
                        resolved_intensity_g_per_kwh, intensity_source, intensity_fetched_at,
                        pue_factor, manual_cost_per_kwh, manual_currency, use_api_for_cost, updated_at
                 FROM projects WHERE id = $1",
            )
            .bind(project_id)
            .fetch_optional(pool)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, format!("Project {project_id} not found in online DB")))
        }
    }
}

// --- Carbon API handlers ---

async fn get_carbon_config_handler(
    Path(project_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let row = get_project_carbon_config(&state, source, project_id).await?;
    Ok(Json(make_carbon_config_response(row)))
}

async fn put_carbon_config_handler(
    Path(project_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
    Json(body): Json<CarbonConfigUpdate>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    
    // Step 1: Resolve lat/lon from address if provided
    let (lat, lon, resolved_address) = if let Some(addr) = &body.address {
        match geocode_address(addr).await {
            Ok((lat, lon, display)) => (Some(lat), Some(lon), Some(display)),
            Err(e) => {
                println!("Geocoding failed for {}: {}", addr, e);
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    format!("Could not find location for '{}'. Please try a more specific address.", addr)
                ));
            }
        }
    } else {
        (body.latitude, body.longitude, body.address.clone())
    };

    // Step 2: Determine carbon intensity
    let current = get_project_carbon_config(&state, source, project_id).await?;
    let api_key = if body.clear_api_key {
        None
    } else {
        body.electricity_maps_api_key.clone()
            .or(current.electricity_maps_api_key.clone())
    };

    let country_code = body.country_code.clone().unwrap_or(current.country_code.clone());

    let (intensity, intensity_src) = if let (Some(lat_val), Some(lon_val), Some(key)) = (lat, lon, api_key.as_deref()) {
        match fetch_electricity_maps_intensity(lat_val, lon_val, key).await {
            Ok(v) => (v, "api".to_string()),
            Err(e) => {
                // If the user explicitly provided an API key or changed the address, we should report the error
                // instead of silently falling back.
                return Err(ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to fetch real-time intensity from Electricity Maps: {}. Please check your API key and location.", e)
                ));
            }
        }
    } else {
        (static_intensity(&country_code), format!("static_{country_code}"))
    };

    let pue = body.pue_factor.unwrap_or(current.pue_factor);
    let manual_cost = body.manual_cost_per_kwh.or(current.manual_cost_per_kwh);
    let manual_currency = body.manual_currency.clone().or(current.manual_currency.clone());
    let use_api_for_cost = body.use_api_for_cost.unwrap_or(current.use_api_for_cost);

    let (api_price, api_currency) = if use_api_for_cost {
        if let (Some(lat_val), Some(lon_val), Some(key)) = (lat, lon, api_key.as_deref()) {
            match fetch_electricity_maps_price(lat_val, lon_val, key).await {
                Ok(v) => (Some(v.0), Some(v.1)),
                Err(_) => (None, None), // Silently skip price if API fails during setup? 
                                       // Or maybe report error if use_api_for_cost was explicitly true
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let final_cost = if use_api_for_cost { api_price } else { manual_cost };
    let final_currency = if use_api_for_cost { api_currency } else { manual_currency };

    let now = chrono::Utc::now().to_rfc3339();

    match source {
        DataSource::Local => {
            sqlx::query(
                "UPDATE projects SET
                    country_code = ?,
                    address = ?,
                    latitude = ?,
                    longitude = ?,
                    electricity_maps_api_key = ?,
                    resolved_intensity_g_per_kwh = ?,
                    intensity_source = ?,
                    intensity_fetched_at = ?,
                    pue_factor = ?,
                    manual_cost_per_kwh = ?,
                    manual_currency = ?,
                    use_api_for_cost = ?,
                    updated_at = ?
                 WHERE id = ?",
            )
            .bind(&country_code)
            .bind(&resolved_address)
            .bind(lat)
            .bind(lon)
            .bind(api_key.as_deref())
            .bind(intensity)
            .bind(&intensity_src)
            .bind(if intensity_src == "api" { Some(now.as_str()) } else { None })
            .bind(pue)
            .bind(final_cost)
            .bind(final_currency)
            .bind(use_api_for_cost)
            .bind(&now)
            .bind(project_id)
            .execute(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;
        }
        DataSource::Online => {
            let pool = state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?;
            sqlx::query(
                "UPDATE projects SET
                    country_code = $1,
                    address = $2,
                    latitude = $3,
                    longitude = $4,
                    electricity_maps_api_key = $5,
                    resolved_intensity_g_per_kwh = $6,
                    intensity_source = $7,
                    intensity_fetched_at = $8,
                    pue_factor = $9,
                    manual_cost_per_kwh = $10,
                    manual_currency = $11,
                    use_api_for_cost = $12,
                    updated_at = $13
                 WHERE id = $14",
            )
            .bind(&country_code)
            .bind(&resolved_address)
            .bind(lat)
            .bind(lon)
            .bind(api_key.as_deref())
            .bind(intensity)
            .bind(&intensity_src)
            .bind(if intensity_src == "api" { Some(now.as_str()) } else { None })
            .bind(pue)
            .bind(final_cost)
            .bind(final_currency)
            .bind(use_api_for_cost)
            .bind(&now)
            .bind(project_id)
            .execute(pool)
            .await
            .map_err(ApiError::internal)?;
        }
    }

    let row = get_project_carbon_config(&state, source, project_id).await?;
    Ok(Json(make_carbon_config_response(row)))
}

async fn post_refresh_carbon_intensity(
    Path(project_id): Path<i64>,
    Query(source_query): Query<SourceQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let source = resolve_source(&state, &source_query)?;
    let current = get_project_carbon_config(&state, source, project_id).await?;
    let (intensity, intensity_src) = match (current.latitude, current.longitude, current.electricity_maps_api_key.as_deref()) {
        (Some(lat), Some(lon), Some(key)) if !key.is_empty() => {
            match fetch_electricity_maps_intensity(lat, lon, key).await {
                Ok(v) => (v, "api".to_string()),
                Err(e) => return Err(ApiError::new(StatusCode::BAD_GATEWAY, format!("Electricity Maps Intensity API error: {e}"))),
            }
        }
        _ => return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "No location and API key configured. Set address and electricity_maps_api_key first.",
        )),
    };

    let (final_cost, final_currency) = if current.use_api_for_cost {
        if let (Some(lat), Some(lon), Some(key)) = (current.latitude, current.longitude, current.electricity_maps_api_key.as_deref()) {
            match fetch_electricity_maps_price(lat, lon, key).await {
                Ok((p, c)) => (Some(p), Some(c)),
                Err(e) => return Err(ApiError::new(StatusCode::BAD_GATEWAY, format!("Electricity Maps Price API error: {e}"))),
            }
        } else {
            (current.manual_cost_per_kwh, current.manual_currency.clone())
        }
    } else {
        (current.manual_cost_per_kwh, current.manual_currency.clone())
    };

    let now = chrono::Utc::now().to_rfc3339();
    match source {
        DataSource::Local => {
            sqlx::query(
                "UPDATE projects SET 
                    resolved_intensity_g_per_kwh = ?, 
                    intensity_source = ?, 
                    intensity_fetched_at = ?, 
                    manual_cost_per_kwh = ?,
                    manual_currency = ?,
                    updated_at = ? 
                 WHERE id = ?",
            )
            .bind(intensity)
            .bind(&intensity_src)
            .bind(&now)
            .bind(final_cost)
            .bind(&final_currency)
            .bind(&now)
            .bind(project_id)
            .execute(&state.local_pool)
            .await
            .map_err(ApiError::internal)?;
        }
        DataSource::Online => {
            let pool = state.online_pool.as_ref().ok_or_else(|| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Online pool missing")
            })?;
            sqlx::query(
                "UPDATE projects SET 
                    resolved_intensity_g_per_kwh = $1, 
                    intensity_source = $2, 
                    intensity_fetched_at = $3, 
                    manual_cost_per_kwh = $4,
                    manual_currency = $5,
                    updated_at = $6 
                 WHERE id = $7",
            )
            .bind(intensity)
            .bind(&intensity_src)
            .bind(&now)
            .bind(final_cost)
            .bind(&final_currency)
            .bind(&now)
            .bind(project_id)
            .execute(pool)
            .await
            .map_err(ApiError::internal)?;
        }
    }

    let row = get_project_carbon_config(&state, source, project_id).await?;
    Ok(Json(make_carbon_config_response(row)))
}

async fn get_carbon_regions_handler() -> impl IntoResponse {
    let regions: Vec<CarbonRegion> = CARBON_REGIONS
        .iter()
        .map(|(code, name, intensity)| CarbonRegion {
            code: code.to_string(),
            name: name.to_string(),
            intensity_g_per_kwh: *intensity,
        })
        .collect();
    Json(regions)
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

    ensure_sqlite_column(
        db,
        "runsummary",
        "total_co2_g",
        "ALTER TABLE runsummary ADD COLUMN total_co2_g REAL",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runsummary",
        "carbon_intensity_used",
        "ALTER TABLE runsummary ADD COLUMN carbon_intensity_used REAL",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runsummary",
        "pue_used",
        "ALTER TABLE runsummary ADD COLUMN pue_used REAL",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runsummary",
        "total_cost",
        "ALTER TABLE runsummary ADD COLUMN total_cost REAL",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runsummary",
        "currency_used",
        "ALTER TABLE runsummary ADD COLUMN currency_used TEXT",
    )
    .await?;
    ensure_sqlite_column(
        db,
        "runs",
        "command",
        "ALTER TABLE runs ADD COLUMN command TEXT",
    )
    .await?;

    // Carbon fields for projects
    ensure_sqlite_column(db, "projects", "country_code", "ALTER TABLE projects ADD COLUMN country_code TEXT NOT NULL DEFAULT 'WORLD'").await?;
    ensure_sqlite_column(db, "projects", "address", "ALTER TABLE projects ADD COLUMN address TEXT").await?;
    ensure_sqlite_column(db, "projects", "latitude", "ALTER TABLE projects ADD COLUMN latitude REAL").await?;
    ensure_sqlite_column(db, "projects", "longitude", "ALTER TABLE projects ADD COLUMN longitude REAL").await?;
    ensure_sqlite_column(db, "projects", "electricity_maps_api_key", "ALTER TABLE projects ADD COLUMN electricity_maps_api_key TEXT").await?;
    ensure_sqlite_column(db, "projects", "resolved_intensity_g_per_kwh", "ALTER TABLE projects ADD COLUMN resolved_intensity_g_per_kwh REAL NOT NULL DEFAULT 494.0").await?;
    ensure_sqlite_column(db, "projects", "intensity_source", "ALTER TABLE projects ADD COLUMN intensity_source TEXT NOT NULL DEFAULT 'static_WORLD'").await?;
    ensure_sqlite_column(db, "projects", "intensity_fetched_at", "ALTER TABLE projects ADD COLUMN intensity_fetched_at TEXT").await?;
    ensure_sqlite_column(db, "projects", "pue_factor", "ALTER TABLE projects ADD COLUMN pue_factor REAL NOT NULL DEFAULT 1.0").await?;
    ensure_sqlite_column(db, "projects", "manual_cost_per_kwh", "ALTER TABLE projects ADD COLUMN manual_cost_per_kwh REAL").await?;
    ensure_sqlite_column(db, "projects", "manual_currency", "ALTER TABLE projects ADD COLUMN manual_currency TEXT").await?;
    ensure_sqlite_column(db, "projects", "use_api_for_cost", "ALTER TABLE projects ADD COLUMN use_api_for_cost BOOLEAN NOT NULL DEFAULT FALSE").await?;
    ensure_sqlite_column(db, "projects", "updated_at", "ALTER TABLE projects ADD COLUMN updated_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'").await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS run_system_info (
            run_id INTEGER PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
            cpu_name TEXT,
            cpu_cores INTEGER,
            ram_gb REAL,
            gpu_count INTEGER,
            gpu_names TEXT
        )",
    )
    .execute(db)
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

    ensure_postgres_column(
        db,
        "runs",
        "command",
        "ALTER TABLE runs ADD COLUMN command TEXT",
    )
    .await?;

    ensure_postgres_column(
        db,
        "runsummary",
        "total_co2_g",
        "ALTER TABLE runsummary ADD COLUMN total_co2_g DOUBLE PRECISION",
    )
    .await?;
    ensure_postgres_column(
        db,
        "runsummary",
        "carbon_intensity_used",
        "ALTER TABLE runsummary ADD COLUMN carbon_intensity_used DOUBLE PRECISION",
    )
    .await?;
    ensure_postgres_column(
        db,
        "runsummary",
        "pue_used",
        "ALTER TABLE runsummary ADD COLUMN pue_used DOUBLE PRECISION",
    )
    .await?;
    ensure_postgres_column(
        db,
        "runsummary",
        "total_cost",
        "ALTER TABLE runsummary ADD COLUMN total_cost DOUBLE PRECISION",
    )
    .await?;
    ensure_postgres_column(
        db,
        "runsummary",
        "currency_used",
        "ALTER TABLE runsummary ADD COLUMN currency_used TEXT",
    )
    .await?;

    // Carbon fields for projects
    ensure_postgres_column(db, "projects", "country_code", "ALTER TABLE projects ADD COLUMN country_code TEXT NOT NULL DEFAULT 'WORLD'").await?;
    ensure_postgres_column(db, "projects", "address", "ALTER TABLE projects ADD COLUMN address TEXT").await?;
    ensure_postgres_column(db, "projects", "latitude", "ALTER TABLE projects ADD COLUMN latitude DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "projects", "longitude", "ALTER TABLE projects ADD COLUMN longitude DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "projects", "electricity_maps_api_key", "ALTER TABLE projects ADD COLUMN electricity_maps_api_key TEXT").await?;
    ensure_postgres_column(db, "projects", "resolved_intensity_g_per_kwh", "ALTER TABLE projects ADD COLUMN resolved_intensity_g_per_kwh DOUBLE PRECISION NOT NULL DEFAULT 494.0").await?;
    ensure_postgres_column(db, "projects", "intensity_source", "ALTER TABLE projects ADD COLUMN intensity_source TEXT NOT NULL DEFAULT 'static_WORLD'").await?;
    ensure_postgres_column(db, "projects", "intensity_fetched_at", "ALTER TABLE projects ADD COLUMN intensity_fetched_at TEXT").await?;
    ensure_postgres_column(db, "projects", "pue_factor", "ALTER TABLE projects ADD COLUMN pue_factor DOUBLE PRECISION NOT NULL DEFAULT 1.0").await?;
    ensure_postgres_column(db, "projects", "manual_cost_per_kwh", "ALTER TABLE projects ADD COLUMN manual_cost_per_kwh DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "projects", "manual_currency", "ALTER TABLE projects ADD COLUMN manual_currency TEXT").await?;
    ensure_postgres_column(db, "projects", "use_api_for_cost", "ALTER TABLE projects ADD COLUMN use_api_for_cost BOOLEAN NOT NULL DEFAULT FALSE").await?;
    ensure_postgres_column(db, "projects", "updated_at", "ALTER TABLE projects ADD COLUMN updated_at TEXT NOT NULL DEFAULT (now()::text)").await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS carbon_config (
            id BIGSERIAL PRIMARY KEY,
            country_code TEXT NOT NULL DEFAULT 'WORLD',
            address TEXT,
            latitude DOUBLE PRECISION,
            longitude DOUBLE PRECISION,
            electricity_maps_api_key TEXT,
            resolved_intensity_g_per_kwh DOUBLE PRECISION NOT NULL DEFAULT 494.0,
            intensity_source TEXT NOT NULL DEFAULT 'static_WORLD',
            intensity_fetched_at TEXT,
            pue_factor DOUBLE PRECISION NOT NULL DEFAULT 1.0,
            updated_at TEXT NOT NULL DEFAULT now()::text
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO carbon_config (id, country_code, resolved_intensity_g_per_kwh, intensity_source, pue_factor, updated_at)
         VALUES (1, 'WORLD', 494.0, 'static_WORLD', 1.0, now()::text)
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS run_system_info (
            run_id BIGINT PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
            cpu_name TEXT,
            cpu_cores INTEGER,
            ram_gb DOUBLE PRECISION,
            gpu_count INTEGER,
            gpu_names TEXT
        )",
    )
    .execute(db)
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
        .route("/api/projects", get(get_projects).post(post_projects))
        .route("/api/project/{project_id}/runs", get(get_runs))
        .route("/api/run/{run_id}", get(get_run))
        .route("/api/run/{run_id}/record_points", get(get_record_points))
        .route("/api/run/{run_id}/summary", get(get_run_summary))
        .route("/api/run/{run_id}/gpu_records", get(get_gpu_records))
        .route("/api/run/{run_id}/socket_records", get(get_socket_records))
        .route("/api/admin/projects", get(get_admin_projects))
        .route("/api/sse", get(sse_handler))
        .route("/api/project/{project_id}/carbon-config", get(get_carbon_config_handler).put(put_carbon_config_handler))
        .route("/api/project/{project_id}/carbon-config/refresh", post(post_refresh_carbon_intensity))
        .route("/api/carbon-regions", get(get_carbon_regions_handler))
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
