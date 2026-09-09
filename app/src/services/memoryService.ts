import { invoke } from "@tauri-apps/api/core";

/**
 * Mirror of `PersonalMemoryRecord` (persistence/personal_memory.rs).
 * Single evolving markdown document per project scope.
 */
export interface PersonalMemoryRecord {
  id: number;
  project_id: string | null;
  content: string;
  /** Monotonic version counter — must be passed back in save/import to detect concurrent edits. */
  version: number;
  last_consolidated_at: number;
  updated_at: number;
}

/**
 * Retrieves the personal memory document for global scope (`projectId: undefined`)
 * or a specific project. Inserts a blank record if none exists yet.
 */
export function getPersonalMemory(projectId?: string): Promise<PersonalMemoryRecord> {
  return invoke("get_personal_memory", { projectId: projectId ?? null });
}

/**
 * Saves direct manual edits to the personal memory document.
 * `expectedVersion` must equal the current record version to prevent overwriting concurrent edits.
 * Rejects with `VoxIpcError::Conflict` on version mismatch.
 */
export function savePersonalMemory(
  content: string,
  expectedVersion: number,
  projectId?: string,
): Promise<PersonalMemoryRecord> {
  return invoke("save_personal_memory", {
    content,
    expectedVersion,
    projectId: projectId ?? null,
  });
}

/**
 * Merges active personal facts or applies directive comments to consolidate the
 * personal memory document via LLM. Emits `PersonalMemoryUpdated` on success.
 */
export function consolidatePersonalMemory(
  comments?: string[],
  projectId?: string,
): Promise<PersonalMemoryRecord> {
  return invoke("consolidate_personal_memory", {
    comments: comments ?? null,
    projectId: projectId ?? null,
  });
}

/** Exports the personal memory document to a local markdown file at `targetPath`. */
export function exportPersonalMemory(targetPath: string, projectId?: string): Promise<void> {
  return invoke("export_personal_memory", {
    targetPath,
    projectId: projectId ?? null,
  });
}

/**
 * Imports and overwrites the personal memory document from an external markdown file.
 * Emits `PersonalMemoryUpdated` on success.
 */
export function importPersonalMemory(
  sourcePath: string,
  projectId?: string,
): Promise<PersonalMemoryRecord> {
  return invoke("import_personal_memory", {
    sourcePath,
    projectId: projectId ?? null,
  });
}
/**
 * Mirror of `FactRecord` (persistence/facts.rs).
 * Represents a single active memory fact extracted from a session.
 *
 * `fact_type` values: "personal" | "objective" | "workdone" | "blocker" | "next_step" | "pitfall"
 * "personal" facts are identity/persistent; all others are session-scoped.
 */
export interface FactRecord {
  id: string;
  session_id: number | null;
  compaction_id: number;
  fact_type: string;
  text: string;
  status: string;
  created_at: number;
  updated_at: number;
}

/**
 * Returns all `status = 'active'` memory facts for graph visualization.
 * Includes every fact_type: personal, objective, workdone, blocker, next_step, pitfall.
 */
export function getActiveFacts(projectId?: string): Promise<FactRecord[]> {
  return invoke("get_active_facts", { projectId: projectId ?? null });
}
