use std::fs::File;

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;
use csv::Writer;

pub struct CsvExporter {
    file_path: String,
    writer: Writer<File>,
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
            self.writer
                .write_record(vec![
                    "ID",
                    "Run ID",
                    "PID",
                    "Timestamp",
                    "CPU%",
                    "CPU(W)",
                    "GPU%",
                    "GPU(W)",
                    "MEM%",
                    "MEM(W)",
                    "IGPU%",
                    "IGPU(W)",
                ])
                .unwrap();
        }
        self.first_record = false;
        self.writer.write_record(&record.to_vec()).unwrap();
        self.writer.flush().unwrap();
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
        })
    }
    async fn create_run(
        &mut self,
        _run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        _started_at: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        Ok(0)
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
