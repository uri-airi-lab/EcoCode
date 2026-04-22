// SQLite exporter — persists measurement records into a SQLite database.

use std::{env, io, path::PathBuf};

use async_trait::async_trait;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};

/// Exports records to a SQLite database file.
///
/// The `records` table is created automatically if it does not exist.
pub struct SqliteExporter {
    db: SqlitePool,
}

impl SqliteExporter {
    pub async fn new() -> Result<SqliteExporter, Box<dyn std::error::Error>> {
        let db_url = Self::resolve_db_url();
        let connect_options: SqliteConnectOptions = db_url.parse()?;
        let connect_options = connect_options.create_if_missing(true).foreign_keys(true);
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await?;

        Self::initialize_schema(&db).await?;

        Ok(SqliteExporter { db })
    }

    fn resolve_db_url() -> String {
        if let Ok(db_url) = env::var("ECOCODE_DB_URL") {
            return db_url;
        }

        let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ecocodeDB.db");
        format!("sqlite://{}", db_path.display())
    }

    async fn initialize_schema(db: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("PRAGMA journal_mode=WAL;").execute(db).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE,
                collaborator_name TEXT
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS project_collaborators (
                project_id INTEGER NOT NULL,
                collaborator_name TEXT NOT NULL,
                PRIMARY KEY (project_id, collaborator_name),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS runs (
                id INTEGER PRIMARY KEY,
                name TEXT,
                project_id INTEGER,
                collaborator_name TEXT,
                status TEXT NOT NULL DEFAULT 'finished',
                started_at TEXT,
                ended_at TEXT,
                command TEXT,
                FOREIGN KEY (project_id) REFERENCES projects(id)
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS records (
                id INTEGER PRIMARY KEY,
                run_id INTEGER,
                pid INTEGER,
                timestamp TEXT,
                cpu_usage REAL,
                cpu_energy REAL,
                gpu_usage REAL,
                gpu_energy REAL,
                mem_usage REAL,
                mem_energy REAL,
                igpu_usage REAL,
                igpu_energy REAL,
                FOREIGN KEY (run_id) REFERENCES runs(id)
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS gpu_records (
                id INTEGER PRIMARY KEY,
                record_id INTEGER NOT NULL,
                device_index INTEGER NOT NULL,
                gpu_usage REAL,
                gpu_energy REAL,
                FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS socket_records (
                id INTEGER PRIMARY KEY,
                record_id INTEGER NOT NULL,
                socket INTEGER NOT NULL,
                domain TEXT NOT NULL,
                energy REAL,
                FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS runsummary (
                id INTEGER PRIMARY KEY,
                run_id INTEGER,
                total_cpu_energy REAL,
                total_gpu_energy REAL,
                total_mem_energy REAL,
                total_igpu_energy REAL,
                FOREIGN KEY (run_id) REFERENCES runs(id)
            )",
        )
        .execute(db)
        .await?;

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
            "ALTER TABLE runs ADD COLUMN command TEXT",
        )
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS run_system_info (
                run_id INTEGER PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
                cpu_name TEXT,
                cpu_cores INTEGER,
                ram_gb REAL,
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
        db: &SqlitePool,
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
        db: &SqlitePool,
        table: &str,
        column: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(db)
            .await?;

        Ok(rows
            .iter()
            .any(|row| row.get::<String, _>("name") == column))
    }

    async fn backfill_project_collaborators(
        db: &SqlitePool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT OR IGNORE INTO project_collaborators (project_id, collaborator_name)
             SELECT id, collaborator_name
             FROM projects
             WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "INSERT OR IGNORE INTO project_collaborators (project_id, collaborator_name)
             SELECT DISTINCT project_id, collaborator_name
             FROM runs
             WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''",
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
            "SELECT collaborator_name
             FROM project_collaborators
             WHERE project_id = ?
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
            "INSERT OR IGNORE INTO project_collaborators (project_id, collaborator_name) VALUES (?, ?)",
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

        sqlx::query("UPDATE projects SET collaborator_name = ? WHERE id = ?")
            .bind(shortcut.as_deref())
            .bind(project_id)
            .execute(&self.db)
            .await?;

        Ok(shortcut)
    }

    async fn sync_all_project_shortcuts(db: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
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
impl Exporter for SqliteExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Local
    }

    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        let result = sqlx::query(
            "INSERT INTO records (run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy, mem_usage, mem_energy, igpu_usage, igpu_energy)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .execute(&self.db)
        .await?;

        let record_id = result.last_insert_rowid();

        // Insert per-GPU records
        for gm in &record.gpu_metrics {
            sqlx::query(
                "INSERT INTO gpu_records (record_id, device_index, gpu_usage, gpu_energy) VALUES (?, ?, ?, ?)",
            )
            .bind(record_id)
            .bind(gm.device_index as i64)
            .bind(gm.gpu_usage)
            .bind(gm.gpu_energy)
            .execute(&self.db)
            .await?;
        }

        // Insert per-socket RAPL records
        for sm in &record.socket_metrics {
            sqlx::query(
                "INSERT INTO socket_records (record_id, socket, domain, energy) VALUES (?, ?, ?, ?)",
            )
            .bind(record_id)
            .bind(sm.socket as i64)
            .bind(&sm.domain)
            .bind(sm.energy)
            .execute(&self.db)
            .await?;
        }

        // Bump user_version so polling readers can detect DB updates.
        let version = record_id.clamp(i32::MIN as i64, i32::MAX as i64);
        let pragma = format!("PRAGMA user_version = {version}");
        sqlx::query(&pragma).execute(&self.db).await?;

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

        let existing_project = sqlx::query("SELECT id FROM projects WHERE name = ?")
            .bind(project_name)
            .fetch_optional(&self.db)
            .await?;

        if let Some(row) = existing_project {
            let id = row.get::<i64, _>("id");
            if let Some(new_collaborator) = collaborator_name {
                self.add_project_collaborator(id, new_collaborator).await?;
            }

            let collaborators = self.load_project_collaborators(id).await?;
            let selected_collaborator_name = if let Some(new_collaborator) = collaborator_name {
                Some(new_collaborator.to_owned())
            } else if collaborators.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Project '{project_name}' does not have any saved collaborators yet. Re-run with --collaborator <name>."
                    ),
                )
                .into());
            } else if collaborators.len() == 1 {
                Some(collaborators[0].clone())
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Project '{project_name}' has multiple collaborators ({:?}). Re-run with --collaborator <name>.",
                        collaborators
                    ),
                )
                .into());
            };

            self.sync_project_shortcut(id, &collaborators).await?;

            return Ok(ProjectInfo {
                id,
                selected_collaborator_name,
                collaborators,
            });
        }

        let collaborator_name = collaborator_name.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Project '{project_name}' does not have a saved collaborator yet. Re-run with --collaborator <name>."
                ),
            )
        })?;

        let result = sqlx::query("INSERT INTO projects (name, collaborator_name) VALUES (?, ?)")
            .bind(project_name)
            .bind(collaborator_name)
            .execute(&self.db)
            .await?;

        let project_id = result.last_insert_rowid();
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
                "A collaborator is required to create a local run.",
            )
        })?;
        let started_at = started_at.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "A start timestamp is required to create a local run.",
            )
        })?;

        let result = sqlx::query(
            "INSERT INTO runs (name, project_id, collaborator_name, status, started_at, command) VALUES (?, ?, ?, 'running', ?, ?)",
        )
        .bind(run_name)
        .bind(project_id)
        .bind(collaborator_name)
        .bind(started_at)
        .bind(command)
        .execute(&self.db)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn save_system_info(
        &mut self,
        run_id: i64,
        info: crate::exporter::SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO run_system_info (run_id, cpu_name, cpu_cores, ram_gb, gpu_count, gpu_names)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (run_id) DO UPDATE SET
                cpu_name = excluded.cpu_name,
                cpu_cores = excluded.cpu_cores,
                ram_gb = excluded.ram_gb,
                gpu_count = excluded.gpu_count,
                gpu_names = excluded.gpu_names",
        )
        .bind(run_id)
        .bind(info.cpu_name)
        .bind(info.cpu_cores)
        .bind(info.ram_gb)
        .bind(info.gpu_count)
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
        sqlx::query("UPDATE runs SET status = ?, ended_at = ? WHERE id = ?")
            .bind(status)
            .bind(ended_at)
            .bind(run_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn run_summary(&mut self, run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        let record_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM records WHERE run_id = ?")
            .bind(run_id)
            .fetch_one(&self.db)
            .await?;

        sqlx::query("DELETE FROM runsummary WHERE run_id = ?")
            .bind(run_id)
            .execute(&self.db)
            .await?;

        if record_count == 0 {
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO runsummary (run_id, total_cpu_energy, total_gpu_energy, total_mem_energy, total_igpu_energy)
            SELECT
                ?,
                COALESCE(SUM(cpu_energy), 0.0),
                COALESCE(SUM(gpu_energy), 0.0),
                COALESCE(SUM(mem_energy), 0.0),
                COALESCE(SUM(igpu_energy), 0.0)
            FROM records
            WHERE run_id = ?",
        )
        .bind(run_id)
        .bind(run_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }
}
