//! Exporter module — defines the output backends for measurement records.
//!
//! Available exporters:
//! - [`terminal`] — pretty-printed table to stdout
//! - [`csv`] — comma-separated values file
//! - [`json`] — JSON array file
//! - [`sqlite`] — SQLite database
//! - [`prometheus`] — Prometheus metrics endpoint via HTTP
//! - [`logfile`] — formatted log file

pub mod csv;
pub mod dual;
pub mod json;
pub mod local;
pub mod logfile;
pub mod online;
pub mod prometheus;
pub mod terminal;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Per-GPU metrics for a single measurement record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpuMetric {
    pub device_index: u32,
    pub gpu_usage: f64,  // percentage (0-100)
    pub gpu_energy: f64, // Joules (Watt-seconds)
}

/// Per-socket RAPL metrics for a single measurement record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocketMetric {
    pub socket: i16,
    pub domain: String, // "cpu", "dram", "igpu"
    pub energy: f64,    // Joules (Watt-seconds)
}

/// Represents a single measurement record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: u32,
    pub run_id: i64,
    pub pid: u32,
    pub timestamp: String,                 // RFC3339 UTC timestamp
    pub cpu_usage: f64,                    // percentage (0-100)
    pub cpu_energy: f64,                   // Joules (Watt-seconds) (aggregate across sockets)
    pub gpu_usage: f64,                    // percentage (0-100) (aggregate across GPUs)
    pub gpu_energy: f64,                   // Joules (Watt-seconds) (aggregate across GPUs)
    pub mem_usage: f64,                    // percentage (0-100), always available from sysinfo
    pub mem_energy: Option<f64>, // Joules (Watt-seconds) — None when DRAM RAPL domain absent on this system
    pub igpu_usage: Option<f64>, // None — no per-process iGPU sensor available
    pub igpu_energy: Option<f64>, // None when iGPU RAPL domain absent on this system
    pub gpu_metrics: Vec<GpuMetric>, // per-GPU breakdown
    pub socket_metrics: Vec<SocketMetric>, // per-socket RAPL breakdown
}

impl Record {
    pub fn new(
        id: u32,
        run_id: i64,
        pid: u32,
        timestamp: String,
        cpu_usage: f64,
        cpu_energy: f64,
        gpu_usage: f64,
        gpu_energy: f64,
        mem_usage: Option<f64>,
        mem_energy: Option<f64>,
        igpu_usage: Option<f64>,
        igpu_energy: Option<f64>,
        gpu_metrics: Vec<GpuMetric>,
        socket_metrics: Vec<SocketMetric>,
    ) -> Record {
        Record {
            id,
            run_id,
            pid,
            timestamp,
            cpu_usage,
            cpu_energy,
            gpu_usage,
            gpu_energy,
            mem_usage: mem_usage.unwrap_or(0.0),
            mem_energy,
            igpu_usage,
            igpu_energy,
            gpu_metrics,
            socket_metrics,
        }
    }
    pub fn to_vec(&self) -> Vec<String> {
        let mut v = vec![
            self.id.to_string(),
            self.run_id.to_string(),
            self.pid.to_string(),
            self.timestamp.clone(),
            self.cpu_usage.to_string(),
            self.cpu_energy.to_string(),
            self.gpu_usage.to_string(),
            self.gpu_energy.to_string(),
            self.mem_usage.to_string(),
            self.mem_energy
                .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
            self.igpu_usage
                .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
            self.igpu_energy
                .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
        ];
        for gm in &self.gpu_metrics {
            v.push(gm.gpu_usage.to_string());
            v.push(gm.gpu_energy.to_string());
        }
        v
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExporterType {
    Terminal,
    Csv,
    Json,
    Local,
    Online,
    Prometheus,
    LogFile,
    // Http, // Renamed to Online
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub ram_gb: f64,
    pub gpu_count: u32,
    pub gpu_names: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectInfo {
    pub id: i64,
    pub selected_collaborator_name: Option<String>,
    pub collaborators: Vec<String>,
    pub is_new: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CarbonConfig {
    pub resolved_intensity_g_per_kwh: f64,
    pub pue_factor: f64,
    pub intensity_source: String,
    pub manual_cost_per_kwh: Option<f64>,
    pub manual_currency: Option<String>,
    pub use_api_for_cost: bool,
}

/// Trait for different export formats
#[async_trait(?Send)]
pub trait Exporter {
    fn exporter_type(&self) -> ExporterType; // Returns "terminal", "csv", "json", etc.
    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>>;
    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn get_or_create_project(
        &mut self,
        project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>>;
    async fn create_run(
        &mut self,
        run_name: &str,
        project_id: i64,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>>;
    async fn save_system_info(
        &mut self,
        run_id: i64,
        info: SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn complete_run(
        &mut self,
        run_id: i64,
        status: &str,
        ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn update_project_config(
        &mut self,
        _project_id: i64,
        _intensity: Option<f64>,
        _price: Option<f64>,
        _address: Option<String>,
        _use_api_for_cost: Option<bool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    async fn run_summary(&mut self, run_id: i64) -> Result<(), Box<dyn std::error::Error>>;
    async fn get_carbon_config(
        &mut self,
        _project_id: i64,
    ) -> Result<CarbonConfig, Box<dyn std::error::Error>> {
        // Default fallback for exporters that don't have a database connection (CSV, JSON, etc.)
        Ok(CarbonConfig {
            resolved_intensity_g_per_kwh: 445.0, // IEA 2025 global average fallback
            pue_factor: 1.0,
            intensity_source: "static_WORLD_fallback".to_string(),
            manual_cost_per_kwh: None,
            manual_currency: Some("USD".to_string()),
            use_api_for_cost: false,
        })
    }
}
