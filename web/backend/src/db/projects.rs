use std::collections::HashMap;
use crate::error::ApiError;
use crate::models::project::{Project, AdminProjectRow, ProjectCollaboratorRow, AdminProjectInfo};
use crate::models::run::{AdminRunRow, RunActivityRow, AdminRunInfo};
use crate::state::AppState;

/// Fetches all projects that the specified user has access to through their workspaces.
pub async fn fetch_projects(state: &AppState, user_id: i64) -> Result<Vec<Project>, ApiError> {
    sqlx::query_as::<_, Project>(
        "SELECT p.id, p.name, w.slug AS workspace_slug
         FROM projects p
         JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
         JOIN workspaces w ON p.workspace_id = w.id
         WHERE wm.user_id = $1
         ORDER BY p.name ASC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)
}

/// Fetches an aggregated view of all projects, their collaborators, and their nested runs.
/// Used for the administrative dashboard.
pub async fn fetch_admin_projects(
    state: &AppState,
) -> Result<Vec<AdminProjectInfo>, ApiError> {
    let projects = sqlx::query_as::<_, AdminProjectRow>(
        "SELECT id, name, collaborator_name FROM projects ORDER BY name ASC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let runs = sqlx::query_as::<_, AdminRunRow>(
        "SELECT id, name, project_id, collaborator_name, status, started_at, ended_at, command
         FROM runs
         ORDER BY project_id ASC, started_at DESC, id DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let activities = sqlx::query_as::<_, RunActivityRow>(
        "SELECT run_id, MAX(timestamp)::text AS last_record_at
         FROM records
         GROUP BY run_id",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let collaborator_rows = sqlx::query_as::<_, ProjectCollaboratorRow>(
        "SELECT project_id, collaborator_name
         FROM project_collaborators
         ORDER BY project_id ASC, collaborator_name ASC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let activity_by_run: HashMap<i64, Option<String>> = activities
        .into_iter()
        .map(|activity| (activity.run_id, activity.last_record_at))
        .collect();

    let mut collaborators_by_project: HashMap<i64, Vec<String>> = HashMap::new();
    for collaborator in collaborator_rows {
        collaborators_by_project
            .entry(collaborator.project_id)
            .or_default()
            .push(collaborator.collaborator_name);
    }

    let mut runs_by_project: HashMap<i64, Vec<AdminRunInfo>> = HashMap::new();
    for run in runs {
        runs_by_project
            .entry(run.project_id)
            .or_default()
            .push(AdminRunInfo {
                run_id: run.id,
                run_name: run.name,
                collaborator_name: run.collaborator_name,
                status: run.status.unwrap_or_else(|| "finished".to_string()),
                started_at: run.started_at,
                ended_at: run.ended_at,
                last_record_at: activity_by_run.get(&run.id).cloned().flatten(),
                command: run.command,
            });
    }

    Ok(projects
        .into_iter()
        .map(|project| {
            let project_runs = runs_by_project.remove(&project.id).unwrap_or_default();
            let latest_run = project_runs.first().cloned();
            let active_runs = project_runs
                .iter()
                .filter(|run| run.status == "running")
                .cloned()
                .collect();
            let collaborators = collaborators_by_project
                .remove(&project.id)
                .unwrap_or_default();

            AdminProjectInfo {
                id: project.id,
                name: project.name,
                default_collaborator: project.collaborator_name,
                collaborators,
                active_runs,
                latest_run,
            }
        })
        .collect())
}
