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
  is_active?: number;
  last_consolidated_at: number;
  updated_at: number;
}

/**
 * Retrieves the active personal memory document for global scope (`projectId: undefined`)
 * or a specific project. Inserts a blank record if none exists yet.
 */
export function getPersonalMemory(projectId?: string): Promise<PersonalMemoryRecord> {
  return invoke("get_personal_memory", { projectId: projectId ?? null });
}

/**
 * Returns all historical revisions of the personal memory document, ordered by version DESC.
 */
export function getPersonalMemoryVersions(projectId?: string): Promise<PersonalMemoryRecord[]> {
  return invoke("get_personal_memory_versions", { projectId: projectId ?? null });
}

/**
 * Promotes a specific historical version to active status.
 */
export function setActivePersonalMemoryVersion(
  version: number,
  projectId?: string,
): Promise<PersonalMemoryRecord> {
  return invoke("set_active_personal_memory_version", {
    version,
    projectId: projectId ?? null,
  });
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

export type ConsolidationConflictPolicy = "prompt" | "pause_compaction" | "queue";

/**
 * Merges active personal facts or applies directive comments to consolidate the
 * personal memory document via LLM. Emits `PersonalMemoryUpdated` on success.
 */
export function consolidatePersonalMemory(
  comments?: string[],
  projectId?: string,
  conflictPolicy?: ConsolidationConflictPolicy,
): Promise<PersonalMemoryRecord> {
  return invoke("consolidate_personal_memory", {
    comments: comments ?? null,
    projectId: projectId ?? null,
    conflictPolicy: conflictPolicy ?? null,
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
