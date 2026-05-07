use serde::Deserialize;

/// Payload for starting a new run ingestion.
#[derive(Deserialize)]
pub struct IngestRunPayload {
    pub project_id: Option<i64>,
    pub name: String,
    pub collaborator_name: Option<String>,
    pub started_at: Option<String>,
    pub command: Option<String>,
    pub exporter_type: Option<String>,
}

/// Nested payload for GPU metrics inside an ingestion record.
#[derive(Deserialize)]
pub struct IngestGpuMetric {
    pub device_index: i32,
    pub gpu_usage: f64,
    pub gpu_energy: f64,
}

/// Nested payload for Socket metrics inside an ingestion record.
#[derive(Deserialize)]
pub struct IngestSocketMetric {
    pub socket: i32,
    pub domain: String,
    pub energy: f64,
}

/// Payload for a single time-series ingestion record point.
#[derive(Deserialize)]
pub struct IngestRecordData {
    pub pid: i64,
    pub timestamp: String,
    pub cpu_usage: f64,
    pub cpu_energy: f64,
    pub gpu_usage: f64,
    pub gpu_energy: f64,
    pub mem_usage: f64,
    pub mem_energy: Option<f64>,
    pub igpu_usage: Option<f64>,
    pub igpu_energy: Option<f64>,
    pub gpu_metrics: Vec<IngestGpuMetric>,
    pub socket_metrics: Vec<IngestSocketMetric>,
    // ML semantic fields — sent by the Python SDK
    pub phase: Option<String>,
    pub epoch_num: Option<i64>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    pub accuracy: Option<f64>,
    pub custom_metrics: Option<serde_json::Value>,
}

/// Payload for reporting system information during a run.
#[derive(Deserialize)]
pub struct IngestSystemInfo {
    pub cpu_name: String,
    pub cpu_cores: i32,
    pub ram_gb: f64,
    pub gpu_count: i32,
    pub gpu_names: String,
}

/// Payload to mark a run as completed or failed.
#[derive(Deserialize)]
pub struct IngestCompletePayload {
    pub status: String,
    pub ended_at: Option<String>,
}
