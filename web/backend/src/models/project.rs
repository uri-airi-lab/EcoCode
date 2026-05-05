use super::run::AdminRunInfo;
use serde::{Deserialize, Serialize};

/// Payload for creating a new project.
#[derive(Deserialize)]
pub struct CreateProjectPayload {
    pub name: String,
    pub workspace_id: i64,
    pub intensity: Option<f64>,
    pub cost: Option<f64>,
    pub currency: Option<String>,
    pub address: Option<String>,
    pub use_api_for_cost: Option<bool>,
}

/// Represents a project entity returned to the client.
#[derive(Serialize, Clone, sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub workspace_slug: String,
}

/// Represents a row from the projects table in admin queries.
#[derive(Clone, sqlx::FromRow)]
pub struct AdminProjectRow {
    pub id: i64,
    pub name: String,
    pub collaborator_name: Option<String>,
}

/// Represents a project collaborator linked to a project.
#[derive(Clone, sqlx::FromRow)]
pub struct ProjectCollaboratorRow {
    pub project_id: i64,
    pub collaborator_name: String,
}

/// Represents an aggregated project view for admin users, containing all its runs.
#[derive(Serialize, Clone)]
pub struct AdminProjectInfo {
    pub id: i64,
    pub name: String,
    pub default_collaborator: Option<String>,
    pub collaborators: Vec<String>,
    pub active_runs: Vec<AdminRunInfo>,
    pub latest_run: Option<AdminRunInfo>,
}
