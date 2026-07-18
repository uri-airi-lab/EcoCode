//! Exporter module — defines the output backends for measurement records.
//!
//! Available exporters:
//! - [`terminal`] — pretty-printed table to stdout
//! - [`csv`] — comma-separated values file
//! - [`json`] — JSON array file
//! - [`online`] — EcoCode SaaS platform (requires API key)
//! - [`prometheus`] — Prometheus metrics endpoint via HTTP
//! - [`logfile`] — formatted log file

pub mod csv;
pub mod json;
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
    pub elapsed_secs: f64, // seconds since previous sample (used by exporters to compute Watts)
    pub cpu_usage: f64,    // percentage (0-100)
    pub cpu_energy: f64,   // Joules (Watt-seconds) (aggregate across sockets)
    pub gpu_usage: f64,    // percentage (0-100) (aggregate across GPUs)
    pub gpu_energy: f64,   // Joules (Watt-seconds) (aggregate across GPUs)
    pub mem_usage: f64,    // percentage (0-100), always available from sysinfo
    pub mem_energy: Option<f64>, // Joules (Watt-seconds) — None when DRAM RAPL domain absent on this system
    pub igpu_usage: Option<f64>, // None — no per-process iGPU sensor available
    pub igpu_energy: Option<f64>, // None when iGPU RAPL domain absent on this system
    pub gpu_metrics: Vec<GpuMetric>, // per-GPU breakdown
    pub socket_metrics: Vec<SocketMetric>, // per-socket RAPL breakdown
    // ML semantic fields — populated by the Python SDK via Unix socket in attached mode
    pub phase: Option<String>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
}

impl Record {
    pub fn new(
        id: u32,
        run_id: i64,
        pid: u32,
        timestamp: String,
        elapsed_secs: f64,
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
        phase: Option<String>,
        tokens_processed: Option<i64>,
        training_step: Option<i64>,
        loss: Option<f64>,
        learning_rate: Option<f64>,
        sequence_length: Option<i32>,
    ) -> Record {
        Record {
            id,
            run_id,
            pid,
            timestamp,
            elapsed_secs,
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
            phase,
            tokens_processed,
            training_step,
            loss,
            learning_rate,
            sequence_length,
        }
    }
    pub fn to_vec(&self) -> Vec<String> {
        // Divide Joule values by elapsed_secs to produce average Watts per interval.
        let es = self.elapsed_secs.max(0.001);
        let mut v = vec![
            self.id.to_string(),
            self.run_id.to_string(),
            self.pid.to_string(),
            self.timestamp.clone(),
            self.cpu_usage.to_string(),
            (self.cpu_energy / es).to_string(),
            self.gpu_usage.to_string(),
            (self.gpu_energy / es).to_string(),
            self.mem_usage.to_string(),
            self.mem_energy
                .map_or_else(|| "N/A".to_string(), |j| (j / es).to_string()),
            self.igpu_usage
                .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
            self.igpu_energy
                .map_or_else(|| "N/A".to_string(), |j| (j / es).to_string()),
        ];
        for gm in &self.gpu_metrics {
            v.push(gm.gpu_usage.to_string());
            v.push((gm.gpu_energy / es).to_string());
        }
        v.push(self.phase.clone().unwrap_or_else(|| "N/A".to_string()));
        v.push(
            self.tokens_processed
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.training_step
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.loss
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.learning_rate
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.sequence_length
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExporterType {
    Terminal,
    Csv,
    Json,
    Online,
    Prometheus,
    LogFile,
}

impl std::fmt::Display for ExporterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExporterType::Terminal => write!(f, "terminal"),
            ExporterType::Csv => write!(f, "csv"),
            ExporterType::Json => write!(f, "json"),
            ExporterType::Online => write!(f, "online"),
            ExporterType::Prometheus => write!(f, "prometheus"),
            ExporterType::LogFile => write!(f, "logfile"),
        }
    }
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
    #[allow(dead_code)]
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

/// Helper struct for local exporters to track energy, carbon, and costs
#[derive(Clone, Debug, Default)]
pub struct LocalSummaryTracker {
    pub total_cpu_energy: f64,
    pub total_gpu_energy: f64,
    pub total_mem_energy: f64,
    pub total_igpu_energy: f64,
    pub total_duration_secs: f64,
    pub intensity: Option<f64>,
    pub price: Option<f64>,
}

impl LocalSummaryTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_record(&mut self, record: &Record) {
        self.total_cpu_energy += record.cpu_energy;
        self.total_gpu_energy += record.gpu_energy;
        self.total_mem_energy += record.mem_energy.unwrap_or(0.0);
        self.total_igpu_energy += record.igpu_energy.unwrap_or(0.0);
        self.total_duration_secs += record.elapsed_secs;
    }

    pub fn update_config(&mut self, intensity: Option<f64>, price: Option<f64>) {
        if intensity.is_some() {
            self.intensity = intensity;
        }
        if price.is_some() {
            self.price = price;
        }
    }

    pub fn print_summary(&self) {
        let total_j = self.total_cpu_energy + self.total_gpu_energy + self.total_mem_energy + self.total_igpu_energy;
        let total_kwh = total_j / 3_600_000.0;

        let intensity_val = self.intensity.unwrap_or(445.0);
        let carbon_g = total_kwh * intensity_val;

        println!();
        println!("╔════════════════════════════════════════════════════╗");
        println!("║                EcoCode Run Summary                 ║");
        println!("╠════════════════════════════════════════════════════╣");

        let secs = self.total_duration_secs.round() as u64;
        let mins = secs / 60;
        let hours = mins / 60;
        let duration_str = if hours > 0 {
            format!("{}h {}m {}s", hours, mins % 60, secs % 60)
        } else if mins > 0 {
            format!("{}m {}s", mins, secs % 60)
        } else {
            format!("{}s", secs)
        };
        println!("║  Total Duration:   {:31}║", duration_str);

        let cpu_wh = self.total_cpu_energy / 3600.0;
        let gpu_wh = self.total_gpu_energy / 3600.0;
        let mem_wh = self.total_mem_energy / 3600.0;
        let total_wh = total_j / 3600.0;

        println!("║  Total CPU Energy: {:31}║", format!("{:.4} Wh ({:.6} kWh)", cpu_wh, cpu_wh / 1000.0));
        println!("║  Total GPU Energy: {:31}║", format!("{:.4} Wh ({:.6} kWh)", gpu_wh, gpu_wh / 1000.0));
        println!("║  Total MEM Energy: {:31}║", format!("{:.4} Wh ({:.6} kWh)", mem_wh, mem_wh / 1000.0));
        println!("║  Total Energy:     {:31}║", format!("{:.4} Wh ({:.6} kWh)", total_wh, total_kwh));

        println!("╠════════════════════════════════════════════════════╣");
        println!("║  Carbon Footprint: {:31}║", format!("{:.3} gCO2eq", carbon_g));
        println!("║  (resolved via: {:34}║", format!("{} at {:.1} g/kWh)", if self.intensity.is_some() { "override" } else { "default" }, intensity_val));

        if let Some(price_val) = self.price {
            let cost = total_kwh * price_val;
            println!("╠════════════════════════════════════════════════════╣");
            println!("║  Electricity Cost: {:31}║", format!("${:.5}", cost));
            println!("║  (resolved via: {:34}║", format!("price at ${:.3}/kWh)", price_val));
        }

        println!("╚════════════════════════════════════════════════════╝");
        println!();
    }
}
