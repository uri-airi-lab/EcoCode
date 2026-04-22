//! Log file exporter — writes measurement records to a log file.
//!
//! Similar to the terminal exporter but outputs to a file instead of stdout.
//! Includes per-GPU and per-socket breakdowns when available.

use std::fs::File;
use std::io::{BufWriter, Write};

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;

pub struct LogFileExporter {
    writer: BufWriter<File>,
    records: Vec<Record>,
    first_record: bool,
}

impl LogFileExporter {
    pub fn new(file_path: String) -> Result<LogFileExporter, Box<dyn std::error::Error>> {
        let file = File::create(&file_path)?;
        let mut writer = BufWriter::new(file);
        writeln!(
            writer,
            "EcoCode Log — started at {}",
            chrono::Utc::now().to_rfc3339()
        )?;
        writeln!(writer, "{}", "=".repeat(140))?;
        writer.flush()?;
        Ok(LogFileExporter {
            writer,
            records: Vec::new(),
            first_record: true,
        })
    }
}

#[async_trait(?Send)]
impl Exporter for LogFileExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::LogFile
    }

    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        self.records.push(record);
        Ok(())
    }

    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(self.writer, "{}", "=".repeat(140))?;
        writeln!(self.writer, "[LOG EXPORT SUMMARY]")?;
        writeln!(self.writer, "Total records: {}", self.records.len())?;

        if !self.records.is_empty() {
            let total_cpu_energy: f64 = self.records.iter().map(|r| r.cpu_energy).sum();
            let total_gpu_energy: f64 = self.records.iter().map(|r| r.gpu_energy).sum();

            // Accumulate optional domains: None means the domain was absent on this system.
            let total_mem_energy: Option<f64> =
                self.records
                    .iter()
                    .fold(None, |acc, r| match (acc, r.mem_energy) {
                        (Some(a), Some(b)) => Some(a + b),
                        (None, Some(b)) => Some(b),
                        (acc, None) => acc,
                    });
            let total_igpu_energy: Option<f64> =
                self.records
                    .iter()
                    .fold(None, |acc, r| match (acc, r.igpu_energy) {
                        (Some(a), Some(b)) => Some(a + b),
                        (None, Some(b)) => Some(b),
                        (acc, None) => acc,
                    });

            let total_energy = total_cpu_energy
                + total_gpu_energy
                + total_mem_energy.unwrap_or(0.0)
                + total_igpu_energy.unwrap_or(0.0);

            writeln!(self.writer, "Total CPU Energy:  {:.4} W", total_cpu_energy)?;
            writeln!(self.writer, "Total GPU Energy:  {:.4} W", total_gpu_energy)?;
            writeln!(
                self.writer,
                "Total MEM Energy:  {}",
                total_mem_energy.map_or_else(|| "N/A".to_string(), |v| format!("{:.4} W", v))
            )?;
            writeln!(
                self.writer,
                "Total iGPU Energy: {}",
                total_igpu_energy.map_or_else(|| "N/A".to_string(), |v| format!("{:.4} W", v))
            )?;
            writeln!(self.writer, "Total Energy:      {:.4} W", total_energy)?;
        }

        writeln!(self.writer, "{}", "=".repeat(140))?;
        self.writer.flush()?;
        Ok(())
    }

    async fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let r = self.records.last().unwrap();

        if self.first_record {
            writeln!(self.writer)?;
            writeln!(
                self.writer,
                "{:<5} {:<8} {:<40} {:<10} {:<10} {:<12} {:<12} {:<12} {:<12} {:<12} {:<12}",
                "ID",
                "PID",
                "Timestamp",
                "CPU%",
                "CPU(W)",
                "GPU%",
                "GPU(W)",
                "MEM%",
                "MEM(W)",
                "IGPU%",
                "IGPU(W)"
            )?;
            writeln!(self.writer, "{}", "-".repeat(140))?;
            self.first_record = false;
        }

        let mem_energy_s = r
            .mem_energy
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
        let igpu_usage_s = r
            .igpu_usage
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
        let igpu_energy_s = r
            .igpu_energy
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));

        writeln!(
            self.writer,
            "{:<5} {:<8} {:<40} {:<10.2} {:<10.2} {:<12.3} {:<12.3} {:<12.3} {:<12} {:<12} {:<12}",
            r.id,
            r.pid,
            r.timestamp,
            r.cpu_usage,
            r.cpu_energy,
            r.gpu_usage,
            r.gpu_energy,
            r.mem_usage,
            mem_energy_s,
            igpu_usage_s,
            igpu_energy_s
        )?;

        // Log per-GPU details if available
        for gm in &r.gpu_metrics {
            writeln!(
                self.writer,
                "      └─ GPU {}: Usage {:.1}%, Energy {:.3} W",
                gm.device_index, gm.gpu_usage, gm.gpu_energy
            )?;
        }

        // Log per-socket details if available
        for sm in &r.socket_metrics {
            writeln!(
                self.writer,
                "      └─ Socket {} ({}): {:.4} W",
                sm.socket, sm.domain, sm.energy
            )?;
        }

        self.writer.flush()?;
        Ok(())
    }

    async fn get_or_create_project(
        &mut self,
        _project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
        Ok(ProjectInfo {
            id: 0,
            selected_collaborator_name: collaborator_name.map(str::to_owned),
            collaborators: collaborator_name.into_iter().map(str::to_owned).collect(),
        })
    }

    async fn create_run(
        &mut self,
        run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        started_at: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        writeln!(
            self.writer,
            "Run: {} | Started: {}",
            run_name,
            started_at.unwrap_or("N/A")
        )?;
        writeln!(self.writer, "{}", "-".repeat(140))?;
        self.writer.flush()?;
        Ok(0)
    }

    async fn complete_run(
        &mut self,
        _run_id: i64,
        status: &str,
        ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(
            self.writer,
            "\nRun completed | Status: {} | Ended: {}",
            status,
            ended_at.unwrap_or("N/A")
        )?;
        self.writer.flush()?;
        Ok(())
    }

    async fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
