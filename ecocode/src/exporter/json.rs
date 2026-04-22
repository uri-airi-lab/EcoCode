use serde_json;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;

pub struct JsonExporter {
    pub file_path: String,
    pub writer: BufWriter<File>,
    pub first_record: bool,
}

impl JsonExporter {
    pub fn new(file_path: String) -> Result<JsonExporter, Box<dyn Error>> {
        let file = File::create(&file_path)?;
        let writer = BufWriter::new(file); // Wrap the file in a buffered writer
        Ok(JsonExporter {
            file_path,
            writer,
            first_record: true,
        })
    }
}

#[async_trait(?Send)]
impl Exporter for JsonExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Json
    }
    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn Error>> {
        // Write a comma if it's not the first record
        if !self.first_record {
            self.writer.write_all(b",")?; // Add a comma between records
        } else {
            // If this is the first record, write the opening bracket for the JSON array
            self.writer.write_all(b"[")?; // Begin the JSON array
            self.first_record = false; // Mark that the first record is processed
        }

        // Serialize the record to JSON and write it
        let json = serde_json::to_string(&record)?;
        self.writer.write_all(json.as_bytes())?;

        Ok(())
    }

    async fn export(&mut self) -> Result<(), Box<dyn Error>> {
        // Write the closing bracket for the JSON array
        self.writer.write_all(b"]")?;
        self.writer.flush()?;

        println!("\n[JSON EXPORT]");
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
