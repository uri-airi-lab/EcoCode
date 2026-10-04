//! Blocking HTTP client for the EcoCode web backend.
//!
//! Endpoints:
//!   - `GET  /api/projects`                       — projects visible to the API key
//!   - `GET  /api/project/{id}/carbon-config`     — carbon and cost settings
//!   - `POST /api/ingest/run`                     — create a new run
//!   - `POST /api/ingest/run/{id}/record`         — add a measurement record
//!   - `POST /api/ingest/run/{id}/system-info`    — save system hardware info
//!   - `POST /api/ingest/run/{id}/complete`       — mark run as finished/failed
//!
//! Uses `reqwest::blocking`, so it runs on plain threads with no async runtime.

use reqwest::blocking::{Client, Response};
use reqwest::header;
use serde::{Deserialize, Serialize};

use crate::types::{CarbonConfig, IngestRecord, Project, SystemInfo};

/// The EcoCode backend. Fixed on purpose: it cannot be changed by environment
/// variables or config files, so records can't be sent elsewhere by mistake.
pub const BASE_URL: &str = "https://ecocode.harmonyhpc.io";

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

type Error = Box<dyn std::error::Error + Send + Sync>;

// ── Implementation ──────────────────────────────────────────────────────────

impl EcoCodeHttpClient {
    /// Client for [`BASE_URL`] authenticated with `api_key`
    /// (e.g. `"ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F"`).
    pub fn new(api_key: &str) -> Result<Self, Error> {
        Self::with_base_url(BASE_URL, api_key)
    }

    /// Client for another backend. Only for tests against a local mock server;
    /// the CLI always uses [`new`](Self::new).
    #[doc(hidden)]
    pub fn with_base_url(base_url: &str, api_key: &str) -> Result<Self, Error> {
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

    /// `GET /api/projects` — all projects visible to the API key.
    pub fn list_projects(&self) -> Result<Vec<Project>, Error> {
        let res = self.client.get(format!("{}/api/projects", self.base_url)).send()?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().unwrap_or_default();
            return Err(format!(
                "Failed to fetch projects (HTTP {}): {}",
                status,
                if body.is_empty() { "empty response".to_string() } else { body }
            )
            .into());
        }
        Ok(res.json()?)
    }

    /// `GET /api/project/{id}/carbon-config` — `None` if the backend has no
    /// settings for this project (non-success status).
    pub fn carbon_config(&self, project_id: i64) -> Result<Option<CarbonConfig>, Error> {
        let url = format!("{}/api/project/{}/carbon-config", self.base_url, project_id);
        let res = self.client.get(url).send()?;
        if !res.status().is_success() {
            return Ok(None);
        }
        Ok(Some(res.json()?))
    }

    /// `POST /api/ingest/run` — create a new run, returns the run ID.
    /// `exporter_type` labels where the run came from (`"online"` for the CLI,
    /// `"sdk"` for the Python SDK).
    pub fn create_run(
        &self,
        project_id: i64,
        name: &str,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
        exporter_type: &str,
    ) -> Result<i64, Error> {
        let url = format!("{}/api/ingest/run", self.base_url);
        let payload = CreateRunPayload {
            project_id,
            name: name.to_string(),
            collaborator_name: collaborator_name.map(|s| s.to_string()),
            started_at: started_at.map(|s| s.to_string()),
            command: command.map(|s| s.to_string()),
            exporter_type: Some(exporter_type.to_string()),
        };

        let res = self.client.post(&url).json(&payload).send()?;
        if !res.status().is_success() {
            return Err(format!(
                "Failed to create run: {} {}",
                res.status(),
                res.text().unwrap_or_default()
            )
            .into());
        }
        let data: CreateRunResponse = res.json()?;
        Ok(data.id)
    }

    /// `POST /api/ingest/run/{id}/record` — add a measurement record.
    pub fn add_record(&self, run_id: i64, record: &IngestRecord) -> Result<(), Error> {
        let url = format!("{}/api/ingest/run/{}/record", self.base_url, run_id);
        check(self.client.post(&url).json(record).send()?, "Failed to ingest record")
    }

    /// `POST /api/ingest/run/{id}/system-info` — save system hardware info.
    pub fn save_system_info(&self, run_id: i64, info: &SystemInfo) -> Result<(), Error> {
        let url = format!("{}/api/ingest/run/{}/system-info", self.base_url, run_id);
        let payload = SystemInfoPayload {
            cpu_name: info.cpu_name.clone(),
            cpu_cores: info.cpu_cores as i32,
            ram_gb: info.ram_gb,
            gpu_count: info.gpu_count as i32,
            gpu_names: info.gpu_names.join(", "),
        };
        check(self.client.post(&url).json(&payload).send()?, "Failed to save system info")
    }

    /// `POST /api/ingest/run/{id}/complete` — mark run as finished/failed/canceled.
    pub fn complete_run(&self, run_id: i64, status: &str, ended_at: Option<&str>) -> Result<(), Error> {
        let url = format!("{}/api/ingest/run/{}/complete", self.base_url, run_id);
        let payload = CompletePayload {
            status: status.to_string(),
            ended_at: ended_at.map(|s| s.to_string()),
        };
        check(self.client.post(&url).json(&payload).send()?, "Failed to complete run")
    }
}

fn check(res: Response, what: &str) -> Result<(), Error> {
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("{}: {}", what, res.status()).into())
    }
}
