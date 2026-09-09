use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use turso::Connection;

/// Strongly-typed row representation of a project grouping.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Fetches all projects ordered by most recently updated first.
pub async fn get_projects(conn: &Connection) -> Result<Vec<ProjectRow>> {
    let mut rows = conn
        .query(
            "SELECT id, name, created_at, updated_at FROM projects ORDER BY updated_at DESC",
            (),
        )
        .await?;

    let mut projects = Vec::new();
    while let Some(row) = rows.next().await? {
        projects.push(ProjectRow {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        });
    }

    Ok(projects)
}

/// Fetches a project by its unique ID.
pub async fn get_project_by_id(conn: &Connection, id: &str) -> Result<Option<ProjectRow>> {
    let mut rows = conn
        .query(
            "SELECT id, name, created_at, updated_at FROM projects WHERE id = ?",
            (id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(ProjectRow {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

/// Creates a new project with the given ID and name.
pub async fn create_project(conn: &Connection, id: &str, name: &str) -> Result<ProjectRow> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO projects (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
        (id.to_string(), name.to_string(), now, now),
    )
    .await?;

    Ok(ProjectRow {
        id: id.to_string(),
        name: name.to_string(),
        created_at: now,
        updated_at: now,
    })
}

/// Renames an existing project.
pub async fn rename_project(conn: &Connection, project_id: &str, new_name: &str) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let affected = conn
        .execute(
            "UPDATE projects SET name = ?, updated_at = ? WHERE id = ?",
            (new_name.to_string(), now, project_id.to_string()),
        )
        .await?;

    if affected == 0 {
        return Err(anyhow!("Project not found: {}", project_id));
    }

    Ok(())
}

/// Deletes a project. Fails if the project is the default project or contains active sessions.
pub async fn delete_project(conn: &Connection, project_id: &str) -> Result<()> {
    if project_id == "default" {
        return Err(anyhow!("Cannot delete the default project"));
    }

    // Check if any sessions exist under this project
    let mut count_rows = conn
        .query(
            "SELECT COUNT(*) FROM sessions WHERE project_id = ?",
            (project_id.to_string(),),
        )
        .await?;

    if let Some(row) = count_rows.next().await? {
        let count: i64 = row.get(0)?;
        if count > 0 {
            return Err(anyhow!(
                "Cannot delete project '{}': {} session(s) exist under it",
                project_id,
                count
            ));
        }
    }

    let affected = conn
        .execute(
            "DELETE FROM projects WHERE id = ?",
            (project_id.to_string(),),
        )
        .await?;

    if affected == 0 {
        return Err(anyhow!("Project not found: {}", project_id));
    }

    Ok(())
}
