import { invoke } from "./transport";

/** Mirror of `ProjectRow` (persistence/projects.rs). */
export interface ProjectRow {
  id: string;
  name: string;
  created_at: number;
  updated_at: number;
}

const projectsInFlight = new Map<string, Promise<ProjectRow[]>>();
const PROJECTS_CACHE_KEY = "__all__";
let cachedProjects: ProjectRow[] | null = null;

function fetchProjectsFromBackend(): Promise<ProjectRow[]> {
  const existing = projectsInFlight.get(PROJECTS_CACHE_KEY);
  if (existing) {
    return existing;
  }
  const promise = invoke<ProjectRow[]>("get_projects")
    .then((data) => {
      cachedProjects = data;
      return data;
    })
    .finally(() => {
      projectsInFlight.delete(PROJECTS_CACHE_KEY);
    });
  projectsInFlight.set(PROJECTS_CACHE_KEY, promise);
  return promise;
}

/**
 * Stale-while-revalidate read: a warm cache is served synchronously so route
 * remounts (SessionPanel, ActiveSessionHeader) paint with zero IPC latency,
 * while a deduped background revalidate refreshes the cache for the next read.
 *
 * Projects change only through the three frontend mutations below, which all
 * invalidate `cachedProjects`. No backend event marks project changes, so
 * invalidation paths must call `refreshProjects` (or let the post-mutation
 * `refresh()` observe the cleared cache).
 */
export function getProjects(): Promise<ProjectRow[]> {
  if (cachedProjects) {
    void fetchProjectsFromBackend().catch(() => {});
    return Promise.resolve(cachedProjects);
  }
  return fetchProjectsFromBackend();
}

/**
 * Forced project-list read for invalidation paths. Always hits the backend
 * (deduped against concurrent in-flight calls) and updates the shared cache.
 */
export function refreshProjects(): Promise<ProjectRow[]> {
  return fetchProjectsFromBackend();
}

/** Creates a new project category. Rejects with VoxIpcError if name is blank. */
export function createProject(name: string): Promise<ProjectRow> {
  cachedProjects = null;
  return invoke<ProjectRow>("create_project", { name });
}

/** Renames a project. Rejects with VoxIpcError if newName is blank. */
export function renameProject(projectId: string, newName: string): Promise<void> {
  cachedProjects = null;
  return invoke("rename_project", { projectId, newName });
}

/**
 * Permanently deletes an empty project.
 * Rejects with VoxIpcError::Conflict if the project still has active sessions.
 */
export function deleteProject(projectId: string): Promise<void> {
  cachedProjects = null;
  return invoke("delete_project", { projectId });
}
