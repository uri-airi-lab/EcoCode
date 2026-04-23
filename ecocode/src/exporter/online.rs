// Exporter for the online EcoCode platform — pushes run data to a
// PostgreSQL / TimeScaleDB instance specified by DATABASE_URL.

use sqlx::{Row, postgres::PgPool};

use dotenv::dotenv;
use std::{env, io};

use crate::carbon::{calculate_co2_g, static_intensity};
use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;

pub struct OnlineExporter {
    db: PgPool,
}

impl OnlineExporter {
    pub async fn new() -> Result<OnlineExporter, Box<dyn std::error::Error>> {
        dotenv().ok();
        let db_url = Self::resolve_db_url();
        let db = PgPool::connect(&db_url).await?;
        Self::initialize_schema(&db).await?;
        Ok(OnlineExporter { db })
    }

    fn resolve_db_url() -> String {
        if let Ok(db_url) = env::var("ECOCODE_ONLINE_DB_URL") {
            return db_url;
        }
        env::var("DATABASE_URL").expect("DATABASE_URL or ECOCODE_ONLINE_DB_URL must be set")
    }

    async fn initialize_schema(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projects (
                id BIGSERIAL PRIMARY KEY,
                name TEXT UNIQUE,
                collaborator_name TEXT
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS project_collaborators (
                project_id BIGINT NOT NULL,
                collaborator_name TEXT NOT NULL,
                PRIMARY KEY (project_id, collaborator_name),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS runs (
                id BIGSERIAL PRIMARY KEY,
                name TEXT,
                project_id BIGINT REFERENCES projects(id),
                collaborator_name TEXT,
                status TEXT NOT NULL DEFAULT 'finished',
                started_at TEXT,
                ended_at TEXT,
                command TEXT
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS records (
                id BIGSERIAL PRIMARY KEY,
                run_id BIGINT REFERENCES runs(id),
                pid BIGINT,
                timestamp TIMESTAMPTZ,
                cpu_usage DOUBLE PRECISION,
                cpu_energy DOUBLE PRECISION,
                gpu_usage DOUBLE PRECISION,
                gpu_energy DOUBLE PRECISION,
                mem_usage DOUBLE PRECISION,
                mem_energy DOUBLE PRECISION,
                igpu_usage DOUBLE PRECISION,
                igpu_energy DOUBLE PRECISION
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS gpu_records (
                id BIGSERIAL PRIMARY KEY,
                record_id BIGINT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
                device_index INTEGER NOT NULL,
                gpu_usage DOUBLE PRECISION,
                gpu_energy DOUBLE PRECISION
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS socket_records (
                id BIGSERIAL PRIMARY KEY,
                record_id BIGINT NOT NULL REFERENCES records(id) ON DELETE CASCADE,
                socket INTEGER NOT NULL,
                domain TEXT NOT NULL,
                energy DOUBLE PRECISION
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS runsummary (
                id BIGSERIAL PRIMARY KEY,
                run_id BIGINT REFERENCES runs(id),
                total_cpu_energy DOUBLE PRECISION,
                total_gpu_energy DOUBLE PRECISION,
                total_mem_energy DOUBLE PRECISION,
                total_igpu_energy DOUBLE PRECISION
            )",
        )
        .execute(db)
        .await?;

        // Forward-compatibility: add columns that may be missing on older deployments.
        Self::ensure_column(
            db,
            "projects",
            "collaborator_name",
            "ALTER TABLE projects ADD COLUMN collaborator_name TEXT",
        )
        .await?;

        Self::ensure_column(
            db,
            "runs",
            "collaborator_name",
            "ALTER TABLE runs ADD COLUMN collaborator_name TEXT",
        )
        .await?;

        Self::ensure_column(
            db,
            "runs",
            "status",
            "ALTER TABLE runs ADD COLUMN status TEXT NOT NULL DEFAULT 'finished'",
        )
        .await?;

        Self::ensure_column(
            db,
            "runs",
            "started_at",
            "ALTER TABLE runs ADD COLUMN started_at TEXT",
        )
        .await?;

        Self::ensure_column(
            db,
            "runs",
            "ended_at",
            "ALTER TABLE runs ADD COLUMN ended_at TEXT",
        )
        .await?;
        
        Self::ensure_column(
            db,
            "runs",
            "command",
            "ALTER TABLE runs ADD COLUMN IF NOT EXISTS command TEXT",
        )
        .await?;

        // Carbon fields for projects
        Self::ensure_column(db, "projects", "country_code", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS country_code TEXT NOT NULL DEFAULT 'WORLD'").await?;
        Self::ensure_column(db, "projects", "address", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS address TEXT").await?;
        Self::ensure_column(db, "projects", "latitude", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION").await?;
        Self::ensure_column(db, "projects", "longitude", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION").await?;
        Self::ensure_column(db, "projects", "electricity_maps_api_key", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS electricity_maps_api_key TEXT").await?;
        Self::ensure_column(db, "projects", "resolved_intensity_g_per_kwh", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS resolved_intensity_g_per_kwh DOUBLE PRECISION NOT NULL DEFAULT 494.0").await?;
        Self::ensure_column(db, "projects", "intensity_source", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS intensity_source TEXT NOT NULL DEFAULT 'static_WORLD'").await?;
        Self::ensure_column(db, "projects", "intensity_fetched_at", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS intensity_fetched_at TEXT").await?;
        Self::ensure_column(db, "projects", "pue_factor", "ALTER TABLE projects ADD COLUMN IF NOT EXISTS pue_factor DOUBLE PRECISION NOT NULL DEFAULT 1.0").await?;

        Self::ensure_column(
            db,
            "runsummary",
            "total_co2_g",
            "ALTER TABLE runsummary ADD COLUMN total_co2_g DOUBLE PRECISION",
        )
        .await?;
        Self::ensure_column(
            db,
            "runsummary",
            "carbon_intensity_used",
            "ALTER TABLE runsummary ADD COLUMN carbon_intensity_used DOUBLE PRECISION",
        )
        .await?;
        Self::ensure_column(
            db,
            "runsummary",
            "pue_used",
            "ALTER TABLE runsummary ADD COLUMN pue_used DOUBLE PRECISION",
        )
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS run_system_info (
                run_id BIGINT PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
                cpu_name TEXT,
                cpu_cores INTEGER,
                ram_gb DOUBLE PRECISION,
                gpu_count INTEGER,
                gpu_names TEXT
            )",
        )
        .execute(db)
        .await?;

        Self::backfill_project_collaborators(db).await?;
        Self::sync_all_project_shortcuts(db).await?;

        Ok(())
    }


    async fn ensure_column(
        db: &PgPool,
        table: &str,
        column: &str,
        alter_sql: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if Self::has_column(db, table, column).await? {
            return Ok(());
        }

        sqlx::query(alter_sql).execute(db).await?;
        Ok(())
    }

    async fn has_column(
        db: &PgPool,
        table: &str,
        column: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let row = sqlx::query(
            "SELECT EXISTS (
                SELECT 1 
                FROM information_schema.columns 
                WHERE table_name = $1 AND column_name = $2
            )",
        )
        .bind(table)
        .bind(column)
        .fetch_one(db)
        .await?;

        Ok(row.get(0))
    }

    async fn backfill_project_collaborators(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO project_collaborators (project_id, collaborator_name)
             SELECT id, collaborator_name FROM projects
             WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''
             ON CONFLICT DO NOTHING",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "INSERT INTO project_collaborators (project_id, collaborator_name)
             SELECT DISTINCT project_id, collaborator_name FROM runs
             WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''
             ON CONFLICT DO NOTHING",
        )
        .execute(db)
        .await?;

        Ok(())
    }

    async fn load_project_collaborators(
        &self,
        project_id: i64,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let collaborators = sqlx::query_scalar::<_, String>(
            "SELECT collaborator_name FROM project_collaborators
             WHERE project_id = $1
             ORDER BY collaborator_name ASC",
        )
        .bind(project_id)
        .fetch_all(&self.db)
        .await?;

        Ok(collaborators)
    }

    async fn add_project_collaborator(
        &self,
        project_id: i64,
        collaborator_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO project_collaborators (project_id, collaborator_name)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(project_id)
        .bind(collaborator_name)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn sync_project_shortcut(
        &self,
        project_id: i64,
        collaborators: &[String],
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let shortcut = if collaborators.len() == 1 {
            Some(collaborators[0].clone())
        } else {
            None
        };
        sqlx::query("UPDATE projects SET collaborator_name = $1 WHERE id = $2")
            .bind(shortcut.as_deref())
            .bind(project_id)
            .execute(&self.db)
            .await?;
        Ok(shortcut)
    }

    async fn sync_all_project_shortcuts(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "UPDATE projects
             SET collaborator_name = (
                 SELECT CASE
                     WHEN COUNT(*) = 1 THEN MAX(collaborator_name)
                     ELSE NULL
                 END
                 FROM project_collaborators
                 WHERE project_id = projects.id
             )",
        )
        .execute(db)
        .await?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl Exporter for OnlineExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Online
    }

    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        let record_id: i64 = sqlx::query_scalar(
            "INSERT INTO records
                (run_id, pid, timestamp,
                 cpu_usage, cpu_energy,
                 gpu_usage, gpu_energy,
                 mem_usage, mem_energy,
                 igpu_usage, igpu_energy)
             VALUES ($1, $2, $3::timestamptz, $4, $5, $6, $7, $8, $9, $10, $11)
             RETURNING id",
        )
        .bind(record.run_id)
        .bind(record.pid as i64)
        .bind(&record.timestamp)
        .bind(record.cpu_usage)
        .bind(record.cpu_energy)
        .bind(record.gpu_usage)
        .bind(record.gpu_energy)
        .bind(record.mem_usage)
        .bind(record.mem_energy)
        .bind(record.igpu_usage)
        .bind(record.igpu_energy)
        .fetch_one(&self.db)
        .await?;

        for gm in &record.gpu_metrics {
            sqlx::query(
                "INSERT INTO gpu_records (record_id, device_index, gpu_usage, gpu_energy)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(record_id)
            .bind(gm.device_index as i64)
            .bind(gm.gpu_usage)
            .bind(gm.gpu_energy)
            .execute(&self.db)
            .await?;
        }

        for sm in &record.socket_metrics {
            sqlx::query(
                "INSERT INTO socket_records (record_id, socket, domain, energy)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(record_id)
            .bind(sm.socket as i64)
            .bind(&sm.domain)
            .bind(sm.energy)
            .execute(&self.db)
            .await?;
        }

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
        project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
        let collaborator_name = collaborator_name
            .map(str::trim)
            .filter(|name| !name.is_empty());

        let existing = sqlx::query("SELECT id FROM projects WHERE name = $1")
            .bind(project_name)
            .fetch_optional(&self.db)
            .await?;

        if let Some(row) = existing {
            let id = row.get::<i64, _>("id");

            if let Some(new_collab) = collaborator_name {
                self.add_project_collaborator(id, new_collab).await?;
            }

            let collaborators = self.load_project_collaborators(id).await?;

            let selected = if let Some(new_collab) = collaborator_name {
                Some(new_collab.to_owned())
            } else if collaborators.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Project '{project_name}' has no saved collaborators. \
                         Re-run with --collaborator <name>."
                    ),
                )
                .into());
            } else if collaborators.len() == 1 {
                Some(collaborators[0].clone())
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Project '{project_name}' has multiple collaborators ({:?}). \
                         Re-run with --collaborator <name>.",
                        collaborators
                    ),
                )
                .into());
            };

            self.sync_project_shortcut(id, &collaborators).await?;
            return Ok(ProjectInfo {
                id,
                selected_collaborator_name: selected,
                collaborators,
            });
        }

        let collaborator_name = collaborator_name.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Project '{project_name}' has no saved collaborator yet. \
                     Re-run with --collaborator <name>."
                ),
            )
        })?;

        let project_id: i64 = sqlx::query_scalar(
            "INSERT INTO projects (name, collaborator_name) VALUES ($1, $2) RETURNING id",
        )
        .bind(project_name)
        .bind(collaborator_name)
        .fetch_one(&self.db)
        .await?;

        self.add_project_collaborator(project_id, collaborator_name)
            .await?;
        let collaborators = self.load_project_collaborators(project_id).await?;
        self.sync_project_shortcut(project_id, &collaborators)
            .await?;

        Ok(ProjectInfo {
            id: project_id,
            selected_collaborator_name: Some(collaborator_name.to_owned()),
            collaborators,
        })
    }

    async fn create_run(
        &mut self,
        run_name: &str,
        project_id: i64,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let collaborator_name = collaborator_name.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "A collaborator is required to create an online run.",
            )
        })?;
        let started_at = started_at.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "A start timestamp is required to create an online run.",
            )
        })?;

        let run_id: i64 = sqlx::query_scalar(
            "INSERT INTO runs (name, project_id, collaborator_name, status, started_at, command)
             VALUES ($1, $2, $3, 'running', $4, $5)
             RETURNING id",
        )
        .bind(run_name)
        .bind(project_id)
        .bind(collaborator_name)
        .bind(started_at)
        .bind(command)
        .fetch_one(&self.db)
        .await?;

        Ok(run_id)
    }

    async fn save_system_info(
        &mut self,
        run_id: i64,
        info: crate::exporter::SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO run_system_info (run_id, cpu_name, cpu_cores, ram_gb, gpu_count, gpu_names)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (run_id) DO UPDATE SET
                cpu_name = EXCLUDED.cpu_name,
                cpu_cores = EXCLUDED.cpu_cores,
                ram_gb = EXCLUDED.ram_gb,
                gpu_count = EXCLUDED.gpu_count,
                gpu_names = EXCLUDED.gpu_names",
        )
        .bind(run_id)
        .bind(info.cpu_name)
        .bind(info.cpu_cores as i32)
        .bind(info.ram_gb)
        .bind(info.gpu_count as i32)
        .bind(info.gpu_names.join(", "))
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn complete_run(
        &mut self,
        run_id: i64,
        status: &str,
        ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("UPDATE runs SET status = $1, ended_at = $2 WHERE id = $3")
            .bind(status)
            .bind(ended_at)
            .bind(run_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn run_summary(&mut self, run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        let record_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM records WHERE run_id = $1")
                .bind(run_id)
                .fetch_one(&self.db)
                .await?;

        sqlx::query("DELETE FROM runsummary WHERE run_id = $1")
            .bind(run_id)
            .execute(&self.db)
            .await?;

        if record_count == 0 {
            return Ok(());
        }

        // Read carbon config from the project associated with this run
        let (intensity, pue, source): (f64, f64, String) = sqlx::query_as::<_, (f64, f64, String)>(
            "SELECT p.resolved_intensity_g_per_kwh, p.pue_factor, p.intensity_source
             FROM projects p
             JOIN runs r ON r.project_id = p.id
             WHERE r.id = $1",
        )
        .bind(run_id)
        .fetch_optional(&self.db)
        .await?
        .unwrap_or_else(|| (static_intensity("WORLD"), 1.0, "static_WORLD".to_string()));

        println!("Carbon footprint calculation: Intensity={:.2} g/kWh (source: {}), PUE={:.2}", 
                 intensity, source, pue);

        // Insert energy totals with placeholder CO2 (updated below)
        sqlx::query(
            "INSERT INTO runsummary
                (run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy,
                 total_co2_g, carbon_intensity_used, pue_used)
             SELECT
                $1,
                COALESCE(SUM(cpu_energy),  0.0),
                COALESCE(SUM(gpu_energy),  0.0),
                COALESCE(SUM(mem_energy),  0.0),
                COALESCE(SUM(igpu_energy), 0.0),
                0.0, $2, $3
             FROM records
             WHERE run_id = $1",
        )
        .bind(run_id)
        .bind(intensity)
        .bind(pue)
        .execute(&self.db)
        .await?;

        // Read back totals to compute CO2
        let (total_cpu, total_gpu, total_mem, total_igpu): (f64, f64, f64, f64) =
            sqlx::query_as::<_, (f64, f64, f64, f64)>(
                "SELECT total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy
                 FROM runsummary WHERE run_id = $1",
            )
            .bind(run_id)
            .fetch_one(&self.db)
            .await?;

        let energy_wh = (total_cpu + total_gpu + total_mem + total_igpu) / 3600.0;
        let co2_g = calculate_co2_g(energy_wh, intensity, pue);

        sqlx::query("UPDATE runsummary SET total_co2_g = $1 WHERE run_id = $2")
            .bind(co2_g)
            .bind(run_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn get_carbon_config(
        &mut self,
        project_id: i64,
    ) -> Result<crate::exporter::CarbonConfig, Box<dyn std::error::Error>> {
        let row: Option<(f64, f64, String)> = sqlx::query_as(
            "SELECT resolved_intensity_g_per_kwh, pue_factor, intensity_source FROM projects WHERE id = $1"
        )
        .bind(project_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some((intensity, pue, source)) = row {
            Ok(crate::exporter::CarbonConfig {
                resolved_intensity_g_per_kwh: intensity,
                pue_factor: pue,
                intensity_source: source,
            })
        } else {
            Ok(crate::exporter::CarbonConfig {
                resolved_intensity_g_per_kwh: static_intensity("WORLD"),
                pue_factor: 1.0,
                intensity_source: "static_WORLD".to_string(),
            })
        }
    }
}
