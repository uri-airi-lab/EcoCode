use serde::Serialize;

/// Represents a monitored run within a project.
#[derive(Serialize, Clone, sqlx::FromRow)]
pub struct Run {
    pub id: i64,
    pub name: String,
    pub project_id: i64,
    pub collaborator_name: Option<String>,
    pub status: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub command: Option<String>,
    pub cpu_name: Option<String>,
    pub cpu_cores: Option<i32>,
    pub ram_gb: Option<f64>,
    pub gpu_count: Option<i32>,
    pub gpu_names: Option<String>,
}

/// Represents the final aggregated metrics for a completed run.
#[derive(Serialize, Clone, sqlx::FromRow, Debug)]
pub struct RunSummary {
    pub run_id: i64,
    pub total_cpu_energy: Option<f64>,
    pub total_gpu_energy: Option<f64>,
    pub total_mem_energy: Option<f64>,
    pub total_igpu_energy: Option<f64>,
    pub total_co2_g: Option<f64>,
    pub carbon_intensity_used: Option<f64>,
    pub pue_used: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency_used: Option<String>,
}

/// Represents a row from the runs table for admin queries.
#[derive(Clone, sqlx::FromRow)]
pub struct AdminRunRow {
    pub id: i64,
    pub name: String,
    pub project_id: i64,
    pub collaborator_name: Option<String>,
    pub status: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub command: Option<String>,
}

/// Represents the latest activity timestamp for a run.
#[derive(Clone, sqlx::FromRow)]
pub struct RunActivityRow {
    pub run_id: i64,
    pub last_record_at: Option<String>,
}

/// Admin view of a single run, including its last activity.
#[derive(Serialize, Clone)]
pub struct AdminRunInfo {
    pub run_id: i64,
    pub run_name: String,
    pub collaborator_name: Option<String>,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub last_record_at: Option<String>,
    pub command: Option<String>,
}
