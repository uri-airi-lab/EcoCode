//! Blocking HTTP client for the EcoCode web backend.
//!
//! Provides a synchronous (non-async) client that implements the 4 ingestion
//! endpoints used by the Python SDK's sampler thread:
//!   1. `POST /api/ingest/run`            — create a new run
//!   2. `POST /api/ingest/run/{id}/record` — add a measurement record
//!   3. `POST /api/ingest/run/{id}/system-info` — save system hardware info
//!   4. `POST /api/ingest/run/{id}/complete`    — mark run as finished/failed
//!   5. `GET  /api/projects`               — list projects (to resolve project_id)
//!
//! Uses `reqwest::blocking` so it can run inside a plain `std::thread` without
//! pulling in a tokio runtime.

use reqwest::blocking::Client;
use reqwest::header;
use serde::{Deserialize, Serialize};

/// Blocking HTTP client for the EcoCode backend.
pub struct EcoCodeHttpClient {
    client: Client,
    base_url: String,
}

// ── Request / Response payloads ─────────────────────────────────────────────

#[derive(Serialize)]
struct CreateRunPayload {
    project_id: i64,
    name: String,
    collaborator_name: Option<String>,
    started_at: Option<String>,
    command: Option<String>,
    exporter_type: Option<String>,
}

#[derive(Deserialize)]
struct CreateRunResponse {
    id: i64,
}

#[derive(Serialize)]
struct SystemInfoPayload {
    cpu_name: String,
    cpu_cores: i32,
    ram_gb: f64,
    gpu_count: i32,
    gpu_names: String,
}

#[derive(Serialize)]
struct CompletePayload {
    status: String,
    ended_at: Option<String>,
}

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

/// A single measurement record sent to the backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub pid: u32,
    pub timestamp: String,
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
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    pub accuracy: Option<f64>,
}

// ── Implementation ──────────────────────────────────────────────────────────

impl EcoCodeHttpClient {
    /// Create a new blocking HTTP client.
    ///
    /// `base_url` — e.g. `"http://localhost:3001"`
    /// `api_key`  — e.g. `"ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F"`
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut headers = header::HeaderMap::new();
        let auth = header::HeaderValue::from_str(&format!("Bearer {}", api_key))?;
        headers.insert(header::AUTHORIZATION, auth);

        let client = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// `POST /api/ingest/run` — create a new run, returns the run ID.
    pub fn create_run(
        &self,
        project_id: i64,
        name: &str,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run", self.base_url);
        let payload = CreateRunPayload {
            project_id,
            name: name.to_string(),
            collaborator_name: collaborator_name.map(|s| s.to_string()),
            started_at: started_at.map(|s| s.to_string()),
            command: command.map(|s| s.to_string()),
            exporter_type: Some("sdk".to_string()),
        };

        let res = self.client.post(&url).json(&payload).send()?;
        if !res.status().is_success() {
            return Err(format!("Failed to create run: {} {}", res.status(), res.text().unwrap_or_default()).into());
        }
        let data: CreateRunResponse = res.json()?;
        Ok(data.id)
    }

    /// `POST /api/ingest/run/{id}/record` — add a measurement record.
    pub fn add_record(
        &self,
        run_id: i64,
        record: &Record,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run/{}/record", self.base_url, run_id);
        let res = self.client.post(&url).json(record).send()?;
        if !res.status().is_success() {
            return Err(format!("Failed to ingest record: {}", res.status()).into());
        }
        Ok(())
    }

    /// `POST /api/ingest/run/{id}/system-info` — save system hardware info.
    pub fn save_system_info(
        &self,
        run_id: i64,
        info: &SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run/{}/system-info", self.base_url, run_id);
        let payload = SystemInfoPayload {
            cpu_name: info.cpu_name.clone(),
            cpu_cores: info.cpu_cores as i32,
            ram_gb: info.ram_gb,
            gpu_count: info.gpu_count as i32,
            gpu_names: info.gpu_names.join(", "),
        };
        let res = self.client.post(&url).json(&payload).send()?;
        if !res.status().is_success() {
            return Err(format!("Failed to save system info: {}", res.status()).into());
        }
        Ok(())
    }

    /// `POST /api/ingest/run/{id}/complete` — mark run as finished/failed/canceled.
    pub fn complete_run(
        &self,
        run_id: i64,
        status: &str,
        ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run/{}/complete", self.base_url, run_id);
        let payload = CompletePayload {
            status: status.to_string(),
            ended_at: ended_at.map(|s| s.to_string()),
        };
        let res = self.client.post(&url).json(&payload).send()?;
        if !res.status().is_success() {
            return Err(format!("Failed to complete run: {}", res.status()).into());
        }
        Ok(())
    }
}
