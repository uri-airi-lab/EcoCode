//! CSV exporter — writes measurement records to a comma-separated values file.
//!
//! A header row is written on the first record, with extra columns appended for
//! each detected GPU (e.g. `GPU0%`, `GPU0(W)`, `GPU1%`, `GPU1(W)`, …).
//! The file is flushed after every record so partial output is preserved if the
//! process terminates unexpectedly.

use std::fs::File;

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;
use csv::Writer;

/// Writes records to a CSV file, with one row per measurement interval.
pub struct CsvExporter {
    file_path: String,
    writer: Writer<File>,
    /// Whether the header row has been written yet.
    pub first_record: bool,
}

impl CsvExporter {
    pub fn new(file_path: String) -> Result<CsvExporter, Box<dyn std::error::Error>> {
        let file = File::create(&file_path)?;
        let writer = csv::Writer::from_writer(file);
        Ok(CsvExporter {
            file_path,
            writer,
            first_record: true,
        })
    }
}

#[async_trait(?Send)]
impl Exporter for CsvExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Csv
    }
    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
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
            self.writer.write_record(&header)?;
            self.first_record = false;
        }
        self.writer.write_record(&record.to_vec())?;
        self.writer.flush()?; // flush csv to file after each record (real-time)
        Ok(())
    }

    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n[CSV EXPORT]");
        println!("Records found in  File: {}", self.file_path);
        Ok(())
    }
    async fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
            is_new: false,
        })
    }
    async fn create_run(
        &mut self,
        _run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        _started_at: Option<&str>,
        _command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        Ok(0)
    }

    async fn save_system_info(
        &mut self,
        _run_id: i64,
        _info: crate::exporter::SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn complete_run(
        &mut self,
        _run_id: i64,
        _status: &str,
        _ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    async fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
