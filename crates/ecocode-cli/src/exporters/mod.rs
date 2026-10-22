//! Exporter module: the output backends for measurement records.
//!
//! Available exporters:
//! - [`terminal`]: pretty-printed table to stdout
//! - [`csv`]: comma-separated values file
//! - [`json`]: JSON array file
//! - [`online`]: EcoCode SaaS platform (requires API key)
//! - [`prometheus`]: Prometheus metrics endpoint via HTTP
//! - [`logfile`]: formatted log file
//!
//! Besides receiving records, an exporter owns the run lifecycle for its
//! destination (project lookup, run creation, completion, summary).

pub mod csv;
pub mod json;
pub mod logfile;
pub mod online;
pub mod prometheus;
pub mod terminal;

use std::error::Error;

pub use ecocode_api::types::{CarbonConfig, SystemInfo};
use ecocode_monitor::Sink;
pub use ecocode_monitor::record::Record;

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

#[derive(Clone, Debug, Default)]
pub struct ProjectInfo {
    pub id: i64,
    pub selected_collaborator_name: Option<String>,
    #[allow(dead_code)]
    pub collaborators: Vec<String>,
}

/// Trait for different export formats
pub trait Exporter {
    fn exporter_type(&self) -> ExporterType; // Returns "terminal", "csv", "json", etc.
    fn add_record(&mut self, record: Record) -> Result<(), Box<dyn Error>>;
    /// Called once at the end of the run.
    fn export(&mut self) -> Result<(), Box<dyn Error>>;
    /// Called after every record.
    fn export_line(&mut self) -> Result<(), Box<dyn Error>>;
    fn get_or_create_project(
        &mut self,
        project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn Error>>;
    fn create_run(
        &mut self,
        run_name: &str,
        project_id: i64,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn Error>>;
    fn save_system_info(&mut self, run_id: i64, info: SystemInfo) -> Result<(), Box<dyn Error>>;
    fn complete_run(&mut self, run_id: i64, status: &str, ended_at: Option<&str>) -> Result<(), Box<dyn Error>>;
    /// Carbon intensity and price given on the command line, for local outputs. Online
    /// exporters ignore it: those runs use the project's settings in the web app.
    fn set_local_config(&mut self, _intensity: Option<f64>, _price: Option<f64>) {}
    fn run_summary(&mut self, run_id: i64) -> Result<(), Box<dyn Error>>;
    fn get_carbon_config(&mut self, _project_id: i64) -> Result<CarbonConfig, Box<dyn Error>> {
        // Default fallback for exporters that don't have a database connection (CSV, JSON, etc.)
        Ok(CarbonConfig::global_fallback())
    }
}

/// Feeds sampler records to an exporter: `add_record` then `export_line`.
pub struct ExporterSink<'a> {
    pub exporter: &'a mut dyn Exporter,
    /// When set, write errors are printed with this prefix and sampling
    /// continues; otherwise the first error ends the run.
    pub log_errors_as: Option<&'static str>,
}

impl Sink for ExporterSink<'_> {
    fn write(&mut self, record: &Record) -> Result<(), Box<dyn Error>> {
        let result = self
            .exporter
            .add_record(record.clone())
            .and_then(|_| self.exporter.export_line());
        match (result, self.log_errors_as) {
            (Err(e), Some(prefix)) => {
                eprintln!("{prefix} Failed to write record: {e}");
                Ok(())
            }
            (result, _) => result,
        }
    }
}

/// One row of a `╔═╗` summary box: the text padded to the box's inner width.
pub fn box_row(text: &str) -> String {
    format!("║{:<52}║", text)
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
        println!("{}", box_row(&format!("  Total Duration:   {}", duration_str)));

        let cpu_wh = self.total_cpu_energy / 3600.0;
        let gpu_wh = self.total_gpu_energy / 3600.0;
        let mem_wh = self.total_mem_energy / 3600.0;
        let total_wh = total_j / 3600.0;

        println!("{}", box_row(&format!("  Total CPU Energy: {:.4} Wh ({:.6} kWh)", cpu_wh, cpu_wh / 1000.0)));
        println!("{}", box_row(&format!("  Total GPU Energy: {:.4} Wh ({:.6} kWh)", gpu_wh, gpu_wh / 1000.0)));
        println!("{}", box_row(&format!("  Total MEM Energy: {:.4} Wh ({:.6} kWh)", mem_wh, mem_wh / 1000.0)));
        println!("{}", box_row(&format!("  Total Energy:     {:.4} Wh ({:.6} kWh)", total_wh, total_kwh)));

        println!("╠════════════════════════════════════════════════════╣");
        println!("{}", box_row(&format!("  Carbon Footprint: {:.3} gCO2eq", carbon_g)));
        println!("{}", box_row(&format!("  (resolved via: {} at {:.1} g/kWh)", if self.intensity.is_some() { "override" } else { "default" }, intensity_val)));

        if let Some(price_val) = self.price {
            let cost = total_kwh * price_val;
            println!("╠════════════════════════════════════════════════════╣");
            println!("{}", box_row(&format!("  Electricity Cost: ${:.5}", cost)));
            println!("{}", box_row(&format!("  (resolved via: price at ${:.3}/kWh)", price_val)));
        }

        println!("╚════════════════════════════════════════════════════╝");
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(cpu: f64, gpu: f64, mem: Option<f64>, igpu: Option<f64>, secs: f64) -> Record {
        Record {
            id: 1,
            run_id: 0,
            pid: 1,
            timestamp: "t".into(),
            elapsed_secs: secs,
            cpu_usage: 0.0,
            cpu_energy: cpu,
            gpu_usage: 0.0,
            gpu_energy: gpu,
            mem_usage: 0.0,
            mem_energy: mem,
            igpu_usage: None,
            igpu_energy: igpu,
            gpu_metrics: vec![],
            socket_metrics: vec![],
            phase: None,
            tokens_processed: None,
            training_step: None,
            loss: None,
            learning_rate: None,
            sequence_length: None,
            accuracy: None,
            custom_metrics: None,
        }
    }

    #[test]
    fn local_summary_sums_all_domains() {
        let mut t = LocalSummaryTracker::new();
        t.add_record(&record(0.05046510577453613, 10.854075, Some(0.0123), Some(0.001), 0.5));
        t.add_record(&record(0.0, 0.0, None, None, 1.0));
        assert_eq!(t.total_cpu_energy, 0.05046510577453613);
        assert_eq!(t.total_gpu_energy, 10.854075);
        assert_eq!(t.total_mem_energy, 0.0123);
        assert_eq!(t.total_igpu_energy, 0.001);
        assert_eq!(t.total_duration_secs, 1.5);
    }
}
