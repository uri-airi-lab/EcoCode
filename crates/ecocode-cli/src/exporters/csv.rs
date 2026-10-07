//! CSV exporter: writes measurement records to a comma-separated values file.
//!
//! A header row is written on the first record, with extra columns appended for
//! each detected GPU (e.g. `GPU0%`, `GPU0(W)`, `GPU1%`, `GPU1(W)`, …).
//! The file is flushed after every record so partial output is preserved if the
//! process terminates unexpectedly.

use std::fs::File;

use crate::exporters::{CarbonConfig, Exporter, ExporterType, ProjectInfo, Record, SystemInfo};
use csv::Writer;

/// Writes records to a CSV file, with one row per measurement interval.
pub struct CsvExporter {
    file_path: String,
    writer: Writer<File>,
    /// Whether the header row has been written yet.
    pub first_record: bool,
    tracker: crate::exporters::LocalSummaryTracker,
}

impl CsvExporter {
    pub fn new(file_path: String) -> Result<CsvExporter, Box<dyn std::error::Error>> {
        let file = File::create(&file_path)?;
        let writer = csv::Writer::from_writer(file);
        Ok(CsvExporter {
            file_path,
            writer,
            first_record: true,
            tracker: crate::exporters::LocalSummaryTracker::new(),
        })
    }
}

impl Exporter for CsvExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Csv
    }
    fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.add_record(&record);
        if self.first_record {
            let mut header: Vec<String> = vec![
                "ID".into(),
                "Run ID".into(),
                "PID".into(),
                "Timestamp".into(),
                "CPU%".into(),
                "CPU(W)".into(),
                "GPU%".into(),
                "GPU(W)".into(),
                "MEM%".into(),
                "MEM(W)".into(),
                "IGPU%".into(),
                "IGPU(W)".into(),
            ];
            for gm in &record.gpu_metrics {
                header.push(format!("GPU{}%", gm.device_index));
                header.push(format!("GPU{}(W)", gm.device_index));
            }
            header.push("Phase".into());
            header.push("TokensProcessed".into());
            header.push("TrainingStep".into());
            header.push("Loss".into());
            header.push("LearningRate".into());
            header.push("SequenceLength".into());
            self.writer.write_record(&header)?;
            self.first_record = false;
        }
        self.writer.write_record(record.to_vec())?;
        self.writer.flush()?; // flush csv to file after each record (real-time)
        Ok(())
    }

    fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n[CSV EXPORT]");
        println!("Records found in  File: {}", self.file_path);
        Ok(())
    }
    fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
        _run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        _started_at: Option<&str>,
        _command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
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
        _status: &str,
        _ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
