//! Log file exporter — writes measurement records to a log file.
//!
//! Similar to the terminal exporter but outputs to a file instead of stdout.
//! Includes per-GPU and per-socket breakdowns when available.

use std::fs::File;
use std::io::{BufWriter, Write};

use crate::exporters::{CarbonConfig, Exporter, ExporterType, ProjectInfo, Record, SystemInfo};

pub struct LogFileExporter {
    writer: BufWriter<File>,
    records: Vec<Record>,
    first_record: bool,
    tracker: crate::exporters::LocalSummaryTracker,
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
            tracker: crate::exporters::LocalSummaryTracker::new(),
        })
    }
}

impl Exporter for LogFileExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::LogFile
    }

    fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.add_record(&record);
        self.records.push(record);
        Ok(())
    }

    fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

            writeln!(
                self.writer,
                "Total CPU Energy:  {:.4} Wh ({:.6} kWh)",
                total_cpu_energy / 3600.0,
                total_cpu_energy / 3_600_000.0
            )?;
            writeln!(
                self.writer,
                "Total GPU Energy:  {:.4} Wh ({:.6} kWh)",
                total_gpu_energy / 3600.0,
                total_gpu_energy / 3_600_000.0
            )?;
            writeln!(
                self.writer,
                "Total MEM Energy:  {}",
                total_mem_energy.map_or_else(
                    || "N/A".to_string(),
                    |v| format!("{:.4} Wh ({:.6} kWh)", v / 3600.0, v / 3_600_000.0)
                )
            )?;
            writeln!(
                self.writer,
                "Total iGPU Energy: {}",
                total_igpu_energy.map_or_else(
                    || "N/A".to_string(),
                    |v| format!("{:.4} Wh ({:.6} kWh)", v / 3600.0, v / 3_600_000.0)
                )
            )?;
            writeln!(
                self.writer,
                "Total Energy:      {:.4} Wh ({:.6} kWh)",
                total_energy / 3600.0,
                total_energy / 3_600_000.0
            )?;

            let total_kwh = total_energy / 3_600_000.0;
            let intensity_val = self.tracker.intensity.unwrap_or(445.0);
            let carbon_g = total_kwh * intensity_val;
            writeln!(self.writer, "Carbon Footprint:  {:.4} gCO2eq (at {:.1} g/kWh)", carbon_g, intensity_val)?;
            if let Some(price_val) = self.tracker.price {
                let cost = total_kwh * price_val;
                writeln!(self.writer, "Electricity Cost:  ${:.5} (at ${:.3}/kWh)", cost, price_val)?;
            }
        }

        writeln!(self.writer, "{}", "=".repeat(140))?;
        self.writer.flush()?;
        Ok(())
    }

    fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(r) = self.records.last() else { return Ok(()) };

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

        let es = r.elapsed_secs.max(0.001);
        let mem_energy_s = r
            .mem_energy
            .map_or_else(|| "N/A".to_string(), |j| format!("{:.3}", j / es));
        let igpu_usage_s = r
            .igpu_usage
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
        let igpu_energy_s = r
            .igpu_energy
            .map_or_else(|| "N/A".to_string(), |j| format!("{:.3}", j / es));

        writeln!(
            self.writer,
            "{:<5} {:<8} {:<40} {:<10.2} {:<10.2} {:<12.3} {:<12.3} {:<12.3} {:<12} {:<12} {:<12}",
            r.id,
            r.pid,
            r.timestamp,
            r.cpu_usage,
            r.cpu_energy / es,
            r.gpu_usage,
            r.gpu_energy / es,
            r.mem_usage,
            mem_energy_s,
            igpu_usage_s,
            igpu_energy_s
        )?;

        // Log per-GPU details if available
        for gm in &r.gpu_metrics {
            writeln!(
                self.writer,
                "      └─ GPU {}: Usage {:.1}%, Power {:.3} W",
                gm.device_index,
                gm.gpu_usage,
                gm.gpu_energy / es
            )?;
        }

        // Log per-socket details if available
        for sm in &r.socket_metrics {
            writeln!(
                self.writer,
                "      └─ Socket {} ({}): {:.4} W",
                sm.socket,
                sm.domain,
                sm.energy / es
            )?;
        }

        if r.phase.is_some()
            || r.tokens_processed.is_some()
            || r.training_step.is_some()
            || r.loss.is_some()
            || r.learning_rate.is_some()
            || r.sequence_length.is_some()
        {
            let mut parts = Vec::new();
            if let Some(ref p) = r.phase {
                parts.push(format!("phase: {}", p));
            }
            if let Some(t) = r.tokens_processed {
                parts.push(format!("tokens: {}", t));
            }
            if let Some(s) = r.training_step {
                parts.push(format!("step: {}", s));
            }
            if let Some(l) = r.loss {
                parts.push(format!("loss: {:.4}", l));
            }
            if let Some(lr) = r.learning_rate {
                parts.push(format!("lr: {:.2e}", lr));
            }
            if let Some(seq) = r.sequence_length {
                parts.push(format!("seqlen: {}", seq));
            }
            writeln!(self.writer, "      └─ Marker: {}", parts.join(" | "))?;
        }

        self.writer.flush()?;
        Ok(())
    }

    fn get_or_create_project(
        &mut self,
        _project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
        Ok(ProjectInfo {
            id: 0,
            selected_collaborator_name: collaborator_name.map(str::to_owned),
            collaborators: collaborator_name.into_iter().map(str::to_owned).collect(),
            is_new: false,
        })
    }

    fn create_run(
        &mut self,
        run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        writeln!(
            self.writer,
            "Run: {} | Started: {} | Command: {}",
            run_name,
            started_at.unwrap_or("N/A"),
            command.unwrap_or("N/A")
        )?;
        writeln!(self.writer, "{}", "-".repeat(140))?;
        self.writer.flush()?;
        Ok(1)
    }

    fn save_system_info(
        &mut self,
        _run_id: i64,
        _info: SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn complete_run(
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

    fn update_project_config(
        &mut self,
        _project_id: i64,
        intensity: Option<f64>,
        price: Option<f64>,
        _address: Option<String>,
        _use_api_for_cost: Option<bool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.update_config(intensity, price);
        Ok(())
    }

    fn get_carbon_config(
        &mut self,
        _project_id: i64,
    ) -> Result<CarbonConfig, Box<dyn std::error::Error>> {
        let intensity_val = self.tracker.intensity.unwrap_or(445.0);
        Ok(CarbonConfig {
            resolved_intensity_g_per_kwh: intensity_val,
            pue_factor: 1.0,
            intensity_source: if self.tracker.intensity.is_some() { "cli_override".to_string() } else { "static_WORLD_fallback".to_string() },
            manual_cost_per_kwh: self.tracker.price,
            manual_currency: Some("USD".to_string()),
            use_api_for_cost: false,
        })
    }

    fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.print_summary();
        Ok(())
    }
}
