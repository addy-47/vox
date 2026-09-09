use std::sync::Arc;

use tauri::State;

use crate::{
    core::{error::VoxIpcError, state::AppState},
    persistence::projects::{
        create_project as db_create_project, delete_project as db_delete_project,
        get_projects as db_get_projects, rename_project as db_rename_project, ProjectRow,
    },
};

/// Returns all workspace projects ordered newest first.
#[tauri::command]
pub async fn get_projects(state: State<'_, Arc<AppState>>) -> Result<Vec<ProjectRow>, VoxIpcError> {
    db_get_projects(&state.db)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Creates a new project category.
#[tauri::command]
pub async fn create_project(
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<ProjectRow, VoxIpcError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Project name cannot be empty".to_string(),
        ));
    }
    let id = uuid::Uuid::new_v4().to_string();
    db_create_project(&state.db, &id, trimmed)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Renames an existing project folder.
#[tauri::command]
pub async fn rename_project(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    new_name: String,
) -> Result<(), VoxIpcError> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(VoxIpcError::InvalidArgument(
            "Project name cannot be empty".to_string(),
        ));
    }
    db_rename_project(&state.db, &project_id, trimmed)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}

/// Permanently deletes an empty project.
#[tauri::command]
pub async fn delete_project(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> Result<(), VoxIpcError> {
    db_delete_project(&state.db, &project_id)
        .await
        .map_err(|e| VoxIpcError::Database(e.to_string()))
}
