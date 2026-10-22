//! JSON exporter: serialises measurement records to a JSON array file.
//!
//! Records are streamed as the monitoring loop runs: the array is opened on the
//! first record and closed when `export()` is called at the end of the run.
//! Each element is a fully serialised [`Record`], including optional per-GPU and
//! per-socket sub-arrays.

use serde_json;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};

use crate::exporters::{CarbonConfig, Exporter, ExporterType, ProjectInfo, Record, SystemInfo};

/// Writes records to a JSON file as a streaming array (`[{...},{...},...]`).
pub struct JsonExporter {
    pub file_path: String,
    pub writer: BufWriter<File>,
    /// Tracks whether the opening `[` bracket has been written yet.
    pub first_record: bool,
    tracker: crate::exporters::LocalSummaryTracker,
}

impl JsonExporter {
    pub fn new(file_path: String) -> Result<JsonExporter, Box<dyn Error>> {
        let file = File::create(&file_path)?;
        let writer = BufWriter::new(file);
        Ok(JsonExporter {
            file_path,
            writer,
            first_record: true,
            tracker: crate::exporters::LocalSummaryTracker::new(),
        })
    }
}

impl Exporter for JsonExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Json
    }
    fn add_record(&mut self, record: Record) -> Result<(), Box<dyn Error>> {
        self.tracker.add_record(&record);
        if !self.first_record {
            self.writer.write_all(b",")?;
        } else {
            self.writer.write_all(b"[")?;
            self.first_record = false;
        }

        let json = serde_json::to_string(&record)?;
        self.writer.write_all(json.as_bytes())?;

        Ok(())
    }

    fn export(&mut self) -> Result<(), Box<dyn Error>> {
        // No records: the opening bracket was never written.
        self.writer.write_all(if self.first_record { b"[]" } else { b"]" })?;
        self.writer.flush()?;

        println!("\n[JSON EXPORT]");
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
    fn set_local_config(&mut self, intensity: Option<f64>, price: Option<f64>) {
        self.tracker.update_config(intensity, price);
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
        })
    }
    fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.print_summary();
        Ok(())
    }
}
