import { invoke } from "@tauri-apps/api/core";

/** Mirror of `ProjectRow` (persistence/projects.rs). */
export interface ProjectRow {
  id: string;
  name: string;
  created_at: number;
  updated_at: number;
}

/** Returns all workspace projects ordered by most recently updated first. */
export function getProjects(): Promise<ProjectRow[]> {
  return invoke("get_projects");
}

/** Creates a new project category. Rejects with VoxIpcError if name is blank. */
export function createProject(name: string): Promise<ProjectRow> {
  return invoke("create_project", { name });
}

/** Renames an existing project. Rejects with VoxIpcError if newName is blank. */
export function renameProject(projectId: string, newName: string): Promise<void> {
  return invoke("rename_project", { projectId, newName });
}

/**
 * Permanently deletes an empty project.
 * Rejects with VoxIpcError::Conflict if the project still has active sessions.
 */
export function deleteProject(projectId: string): Promise<void> {
  return invoke("delete_project", { projectId });
}
