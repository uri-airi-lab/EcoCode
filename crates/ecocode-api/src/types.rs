//! Data exchanged with the EcoCode backend.
//!
//! Field names are the wire format: they must match the backend's
//! `IngestRecordData` / `IngestSystemInfo` payloads.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// System information sent once at run start.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub ram_gb: f64,
    pub gpu_count: u32,
    pub gpu_names: Vec<String>,
}

/// Per-GPU metrics for a single measurement record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpuMetric {
    pub device_index: u32,
    pub gpu_usage: f64,
    pub gpu_energy: f64,
}

/// Per-socket RAPL metrics for a single measurement record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocketMetric {
    pub socket: i16,
    pub domain: String,
    pub energy: f64,
}

/// A single measurement record sent to `POST /api/ingest/run/{id}/record`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IngestRecord {
    pub pid: u32,
    pub timestamp: String,
    pub elapsed_secs: Option<f64>,
    pub cpu_usage: f64,
    pub cpu_energy: f64,
    pub gpu_usage: f64,
    pub gpu_energy: f64,
    pub mem_usage: f64,
    pub mem_energy: Option<f64>,
    pub igpu_usage: Option<f64>,
    pub igpu_energy: Option<f64>,
    pub gpu_metrics: Vec<GpuMetric>,
    pub socket_metrics: Vec<SocketMetric>,
    pub phase: Option<String>,
    pub epoch_num: Option<i64>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    pub accuracy: Option<f64>,
    pub custom_metrics: Option<HashMap<String, f64>>,
}

/// A project visible to the API key (`GET /api/projects`).
#[derive(Clone, Debug, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: Option<String>,
}

/// Carbon and cost settings of a project (`GET /api/project/{id}/carbon-config`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CarbonConfig {
    pub resolved_intensity_g_per_kwh: f64,
    pub pue_factor: f64,
    pub intensity_source: String,
    pub manual_cost_per_kwh: Option<f64>,
    pub manual_currency: Option<String>,
}

impl CarbonConfig {
    /// Used when no project settings are available: IEA 2025 global average.
    pub fn global_fallback() -> Self {
        CarbonConfig {
            resolved_intensity_g_per_kwh: 445.0,
            pue_factor: 1.0,
            intensity_source: "static_WORLD_fallback".to_string(),
            manual_cost_per_kwh: None,
            manual_currency: Some("USD".to_string()),
        }
    }
}
