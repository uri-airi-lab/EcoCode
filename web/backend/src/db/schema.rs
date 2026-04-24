use sqlx::PgPool;

/// Initializes the entire database schema if it doesn't already exist.
/// Applies missing tables and alters existing tables to add new columns.
pub async fn ensure_postgres_schema(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id BIGSERIAL PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            slug TEXT UNIQUE NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
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
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await?;

    // Workspace-scoped API keys for CLI sync
    ensure_postgres_column(
        db,
        "api_keys",
        "workspace_id",
        "ALTER TABLE api_keys ADD COLUMN workspace_id BIGINT REFERENCES workspaces(id) ON DELETE SET NULL",
    )
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id BIGSERIAL PRIMARY KEY,
            workspace_id BIGINT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
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
            ended_at TEXT
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

    ensure_postgres_column(db, "projects", "collaborator_name", "ALTER TABLE projects ADD COLUMN collaborator_name TEXT").await?;
    ensure_postgres_column(db, "runs", "collaborator_name", "ALTER TABLE runs ADD COLUMN collaborator_name TEXT").await?;
    ensure_postgres_column(db, "runs", "status", "ALTER TABLE runs ADD COLUMN status TEXT NOT NULL DEFAULT 'finished'").await?;
    ensure_postgres_column(db, "runs", "started_at", "ALTER TABLE runs ADD COLUMN started_at TEXT").await?;
    ensure_postgres_column(db, "runs", "ended_at", "ALTER TABLE runs ADD COLUMN ended_at TEXT").await?;
    ensure_postgres_column(db, "runs", "command", "ALTER TABLE runs ADD COLUMN command TEXT").await?;

    ensure_postgres_column(db, "runsummary", "total_co2_g", "ALTER TABLE runsummary ADD COLUMN total_co2_g DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "runsummary", "carbon_intensity_used", "ALTER TABLE runsummary ADD COLUMN carbon_intensity_used DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "runsummary", "pue_used", "ALTER TABLE runsummary ADD COLUMN pue_used DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "runsummary", "total_cost", "ALTER TABLE runsummary ADD COLUMN total_cost DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "runsummary", "currency_used", "ALTER TABLE runsummary ADD COLUMN currency_used TEXT").await?;

    ensure_postgres_column(db, "projects", "workspace_id", "ALTER TABLE projects ADD COLUMN workspace_id BIGINT REFERENCES workspaces(id)").await?;
    ensure_postgres_column(db, "projects", "country_code", "ALTER TABLE projects ADD COLUMN country_code TEXT NOT NULL DEFAULT 'WORLD'").await?;
    ensure_postgres_column(db, "projects", "address", "ALTER TABLE projects ADD COLUMN address TEXT").await?;
    ensure_postgres_column(db, "projects", "latitude", "ALTER TABLE projects ADD COLUMN latitude DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "projects", "longitude", "ALTER TABLE projects ADD COLUMN longitude DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "projects", "electricity_maps_api_key", "ALTER TABLE projects ADD COLUMN electricity_maps_api_key TEXT").await?;
    ensure_postgres_column(db, "projects", "resolved_intensity_g_per_kwh", "ALTER TABLE projects ADD COLUMN resolved_intensity_g_per_kwh DOUBLE PRECISION NOT NULL DEFAULT 494.0").await?;
    ensure_postgres_column(db, "projects", "intensity_source", "ALTER TABLE projects ADD COLUMN intensity_source TEXT NOT NULL DEFAULT 'static_WORLD'").await?;
    ensure_postgres_column(db, "projects", "intensity_fetched_at", "ALTER TABLE projects ADD COLUMN intensity_fetched_at TEXT").await?;
    ensure_postgres_column(db, "projects", "pue_factor", "ALTER TABLE projects ADD COLUMN pue_factor DOUBLE PRECISION NOT NULL DEFAULT 1.0").await?;
    ensure_postgres_column(db, "projects", "manual_cost_per_kwh", "ALTER TABLE projects ADD COLUMN manual_cost_per_kwh DOUBLE PRECISION").await?;
    ensure_postgres_column(db, "projects", "manual_currency", "ALTER TABLE projects ADD COLUMN manual_currency TEXT").await?;
    ensure_postgres_column(db, "projects", "use_api_for_cost", "ALTER TABLE projects ADD COLUMN use_api_for_cost BOOLEAN NOT NULL DEFAULT FALSE").await?;
    ensure_postgres_column(db, "projects", "updated_at", "ALTER TABLE projects ADD COLUMN updated_at TEXT NOT NULL DEFAULT (now()::text)").await?;

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

    backfill_postgres_project_collaborators(db).await?;
    sync_postgres_project_shortcuts(db).await?;

    Ok(())
}

/// Helper function to safely add a new column to a table if it does not already exist.
async fn ensure_postgres_column(
    db: &PgPool,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if has_postgres_column(db, table, column).await? {
        return Ok(());
    }

    sqlx::query(alter_sql).execute(db).await?;
    Ok(())
}

/// Checks the `information_schema` to verify if a column exists in a specific table.
async fn has_postgres_column(
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

    use sqlx::Row;
    Ok(row.get(0))
}

/// Populates the `project_collaborators` junction table from historical runs and projects.
async fn backfill_postgres_project_collaborators(
    db: &PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        "INSERT INTO project_collaborators (project_id, collaborator_name)
         SELECT id, collaborator_name
         FROM projects
         WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''
         ON CONFLICT DO NOTHING",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO project_collaborators (project_id, collaborator_name)
         SELECT DISTINCT project_id, collaborator_name
         FROM runs
         WHERE collaborator_name IS NOT NULL AND TRIM(collaborator_name) <> ''
         ON CONFLICT DO NOTHING",
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Updates the deprecated `collaborator_name` shortcut column on `projects`
/// to the single collaborator if exactly one exists.
async fn sync_postgres_project_shortcuts(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
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
