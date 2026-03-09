// This will be the exporter for online platform EcoCode.com to save run informations online.
// It will push the data to TimeScaleDB to be stored on the cloud.

use sqlx::{Error, postgres::PgPool};

use dotenv::dotenv;
use std::env;

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;

pub struct OnlineExporter {
    db: PgPool,
}

impl OnlineExporter {
    pub async fn new() -> Result<OnlineExporter, Error> {
        dotenv().ok();
        let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");

        let db = PgPool::connect(&db_url).await?; // that's why it needs async

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS records (
                id BIGSERIAL PRIMARY KEY,
                pid BIGINT,
                timestamp TIMESTAMPTZ,
                cpu_usage DOUBLE PRECISION,
                cpu_energy DOUBLE PRECISION,
                gpu_usage DOUBLE PRECISION,
                gpu_energy DOUBLE PRECISION
                mem_usage DOUBLE PRECISION,
                mem_energy DOUBLE PRECISION
            )",
        )
        .execute(&db)
        .await?;

        Ok(OnlineExporter { db })
    }



}

#[async_trait(?Send)]
impl Exporter for OnlineExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Online
    }

        async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO records (run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy)
            VALUES ($1, $2, $3::timestamptz, $4, $5, $6 , $7, $8, $9, $10)",
        )
        .bind(record.run_id)
        .bind(record.pid as i64)
        .bind(record.timestamp)
        .bind(record.cpu_usage)
        .bind(record.cpu_energy)
        .bind(record.gpu_usage)
        .bind(record.gpu_energy)
        .bind(record.mem_usage)
        .bind(record.mem_energy)
        .bind(record.igpu_usage)
        .bind(record.igpu_energy)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
