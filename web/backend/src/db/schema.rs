use sqlx::PgPool;

/// Initializes the entire database schema if it doesn't already exist.
/// Consolidates all tables, columns, indexes, and triggers into a clean, final definition.
pub async fn ensure_postgres_schema(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Core Identity & Access
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id BIGSERIAL PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            full_name TEXT,
            institution TEXT NOT NULL DEFAULT '',
            position TEXT NOT NULL DEFAULT '',
            use_case TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(db)
    .await?;

    // Migrate existing databases that are missing the profile columns
    sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS full_name TEXT").execute(db).await?;
    sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS institution TEXT NOT NULL DEFAULT ''").execute(db).await?;
    sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS position TEXT NOT NULL DEFAULT ''").execute(db).await?;
    sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS use_case TEXT").execute(db).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(db)
    .await?;

    // Drop the global unique constraint on slug if it exists (slugs are now unique per user, not globally)
    sqlx::query(
        "DO $$ BEGIN
           IF EXISTS (
             SELECT 1 FROM pg_constraint WHERE conname = 'workspaces_slug_key'
           ) THEN
             ALTER TABLE workspaces DROP CONSTRAINT workspaces_slug_key;
           END IF;
         END $$"
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS workspace_members (
            workspace_id BIGINT NOT NULL,
            user_id BIGINT NOT NULL,
            role TEXT NOT NULL,
            joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (workspace_id, user_id),
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS api_keys (
            id BIGSERIAL PRIMARY KEY,
            user_id BIGINT NOT NULL,
            workspace_id BIGINT REFERENCES workspaces(id) ON DELETE SET NULL,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    // 2. Projects & Collaborators

    // Drop any old global unique constraint on projects.name (names must only be unique per workspace)
    sqlx::query(
        "DO $$ BEGIN
           IF EXISTS (
             SELECT 1 FROM pg_constraint
             WHERE conname = 'projects_name_key' AND conrelid = 'projects'::regclass
           ) THEN
             ALTER TABLE projects DROP CONSTRAINT projects_name_key;
           END IF;
         END $$"
    )
    .execute(db)
    .await?;

    // Ensure the correct per-workspace unique constraint exists
    sqlx::query(
        "DO $$ BEGIN
           IF NOT EXISTS (
             SELECT 1 FROM pg_constraint
             WHERE conname = 'projects_workspace_id_name_key' AND conrelid = 'projects'::regclass
           ) THEN
             ALTER TABLE projects ADD CONSTRAINT projects_workspace_id_name_key UNIQUE (workspace_id, name);
           END IF;
         END $$"
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id BIGSERIAL PRIMARY KEY,
            workspace_id BIGINT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            name TEXT,
            collaborator_name TEXT,
            country_code TEXT NOT NULL DEFAULT 'WORLD',
            address TEXT,
            latitude DOUBLE PRECISION,
            longitude DOUBLE PRECISION,
            electricity_maps_api_key TEXT,
            resolved_intensity_g_per_kwh DOUBLE PRECISION NOT NULL DEFAULT 445.0,
            intensity_source TEXT NOT NULL DEFAULT 'static_WORLD',
            intensity_fetched_at TEXT,
            pue_factor DOUBLE PRECISION NOT NULL DEFAULT 1.0,
            manual_cost_per_kwh DOUBLE PRECISION,
            manual_currency TEXT,
            use_api_for_cost BOOLEAN NOT NULL DEFAULT FALSE,
            updated_at TEXT NOT NULL DEFAULT (now()::text),
            UNIQUE (workspace_id, name)
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

    // 3. Runs & Systems
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS runs (
            id BIGSERIAL PRIMARY KEY,
            project_id BIGINT REFERENCES projects(id) ON DELETE CASCADE,
            name TEXT,
            collaborator_name TEXT,
            status TEXT NOT NULL DEFAULT 'finished',
            command TEXT,
            started_at TEXT,
            ended_at TEXT
        )",
    )
    .execute(db)
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

    // 4. Measurement Records
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS records (
            id BIGSERIAL PRIMARY KEY,
            run_id BIGINT REFERENCES runs(id) ON DELETE CASCADE,
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

    // 5. Results & Summaries
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS runsummary (
            id BIGSERIAL PRIMARY KEY,
            run_id BIGINT REFERENCES runs(id) ON DELETE CASCADE,
            total_cpu_energy DOUBLE PRECISION,
            total_gpu_energy DOUBLE PRECISION,
            total_mem_energy DOUBLE PRECISION,
            total_igpu_energy DOUBLE PRECISION,
            total_co2_g DOUBLE PRECISION,
            carbon_intensity_used DOUBLE PRECISION,
            pue_used DOUBLE PRECISION,
            total_cost DOUBLE PRECISION,
            currency_used TEXT,
            avg_cpu_usage DOUBLE PRECISION,
            avg_gpu_usage DOUBLE PRECISION,
            avg_mem_usage DOUBLE PRECISION
        )",
    )
    .execute(db)
    .await?;

    // Ensure existing databases have the usage columns
    sqlx::query("ALTER TABLE runsummary ADD COLUMN IF NOT EXISTS avg_cpu_usage DOUBLE PRECISION").execute(db).await?;
    sqlx::query("ALTER TABLE runsummary ADD COLUMN IF NOT EXISTS avg_gpu_usage DOUBLE PRECISION").execute(db).await?;
    sqlx::query("ALTER TABLE runsummary ADD COLUMN IF NOT EXISTS avg_mem_usage DOUBLE PRECISION").execute(db).await?;
    sqlx::query("ALTER TABLE runs ADD COLUMN IF NOT EXISTS exporter_type TEXT").execute(db).await?;

    // 6. Maintenance & Data Backfill
    // Backfill usage averages for existing runs that were created before these columns existed
    sqlx::query(
        "UPDATE runsummary rs
         SET 
            avg_cpu_usage = sub.avg_cpu,
            avg_gpu_usage = sub.avg_gpu,
            avg_mem_usage = sub.avg_mem
         FROM (
            SELECT 
                run_id, 
                COALESCE(AVG(cpu_usage), 0.0) as avg_cpu, 
                COALESCE(AVG(gpu_usage), 0.0) as avg_gpu, 
                COALESCE(AVG(mem_usage), 0.0) as avg_mem 
            FROM records 
            GROUP BY run_id
         ) sub
         WHERE rs.run_id = sub.run_id AND rs.avg_cpu_usage IS NULL"
    )
    .execute(db)
    .await?;

    // SDK ML fields — phase labels, token counts, optimizer steps, loss, lr, sequence length
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS phase TEXT").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS tokens_processed BIGINT").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS training_step BIGINT").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS loss DOUBLE PRECISION").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS learning_rate DOUBLE PRECISION").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS sequence_length INT").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS accuracy DOUBLE PRECISION").execute(db).await?;
    sqlx::query("ALTER TABLE records ADD COLUMN IF NOT EXISTS custom_metrics JSONB").execute(db).await?;

    sqlx::query("ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL").execute(db).await?;
    sqlx::query("ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS key_prefix TEXT").execute(db).await?;

    create_performance_indexes(db).await?;
    // Note: We avoid database-level triggers for SSE notifications due to sqlx connection issues.
    // Notifications are now handled manually in the application logic.

    Ok(())
}

/// Sets up PostgreSQL LISTEN/NOTIFY triggers for SSE live updates.
async fn ensure_sse_notify_triggers(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "CREATE OR REPLACE FUNCTION notify_record_inserted()
         RETURNS trigger AS $$
         BEGIN
           PERFORM pg_notify('record_inserted', NEW.id::text);
           RETURN NEW;
         END;
         $$ LANGUAGE plpgsql;"
    )
    .execute(db)
    .await?;

    sqlx::query(
        "DO $$
         BEGIN
           IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'trg_record_inserted') THEN
             CREATE TRIGGER trg_record_inserted
             AFTER INSERT ON records
             FOR EACH ROW EXECUTE FUNCTION notify_record_inserted();
           END IF;
         END $$;"
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Creates database indexes to improve query performance.
async fn create_performance_indexes(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_runs_project_started ON runs(project_id, started_at DESC, id DESC)").execute(db).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_runs_workspace_access ON runs(started_at DESC, id DESC)").execute(db).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_records_run_timestamp ON records(run_id, timestamp)").execute(db).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_workspace_members_user ON workspace_members(user_id, workspace_id)").execute(db).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_projects_workspace ON projects(workspace_id)").execute(db).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_runsummary_run ON runsummary(run_id)").execute(db).await?;
    Ok(())
}
