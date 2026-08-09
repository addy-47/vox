import { invoke } from "@tauri-apps/api/core";

export interface MemoryFactEntry {
  id: string;
  collection: string;
  fact: string;
  source: string;
  created_at: number;
  is_superseded: boolean;
  conflict_count: number;
  supports_count: number;
}

export interface MemoryRelationEntry {
  id: number;
  from_id: string;
  to_id: string;
  relation: string;
  source: string;
  created_at: number;
}

export interface MemoryQueueItem {
  id: number;
  fact: string;
  collection: string;
  source: string;
  session_id: string;
  status: "staged_pending" | "dedup_pass" | "nli_evaluated" | "paused" | "failed";
  attempts: number;
  error_msg?: string;
  created_at: number;
}

export interface MemoryQueueSummary {
  staged_pending: number;
  dedup_pass: number;
  nli_evaluated: number;
  paused: number;
  failed: number;
  recent_items: MemoryQueueItem[];
}

export interface MemoryStats {
  pending_sessions: number;
  embedded_sessions: number;
  total_episodes: number;
  personal_memories: number;
  history_entries: number;
}

/**
 * Fetches all memory fact nodes for the memory graph.
 */
export async function getMemoryGraph(): Promise<MemoryFactEntry[]> {
  try {
    return await invoke<MemoryFactEntry[]>("get_memory_graph");
  } catch (err) {
    console.error("Failed to fetch memory graph:", err);
    return [];
  }
}

/**
 * Fetches all directed graph relation edges.
 */
export async function getMemoryRelations(): Promise<MemoryRelationEntry[]> {
  try {
    return await invoke<MemoryRelationEntry[]>("get_memory_relations");
  } catch (err) {
    console.error("Failed to fetch memory relations:", err);
    return [];
  }
}

/**
 * Fetches current memory pipeline queue status and counts.
 */
export async function getMemoryQueueStatus(): Promise<MemoryQueueSummary | null> {
  try {
    return await invoke<MemoryQueueSummary>("get_memory_queue_status");
  } catch (err) {
    console.error("Failed to fetch memory queue status:", err);
    return null;
  }
}

/**
 * Fetches general memory system statistics.
 */
export async function getMemoryStats(): Promise<MemoryStats | null> {
  try {
    return await invoke<MemoryStats>("get_memory_stats");
  } catch (err) {
    console.error("Failed to fetch memory stats:", err);
    return null;
  }
}

/**
 * Triggers a manual memory pipeline consolidation cycle.
 */
export async function triggerMemoryConsolidation(): Promise<number> {
  try {
    return await invoke<number>("trigger_memory_consolidation");
  } catch (err) {
    console.error("Failed to trigger memory consolidation:", err);
    throw err;
  }
}

/**
 * Edits an existing memory fact (supersedes old fact).
 */
export async function editMemoryFact(
  oldFactId: string,
  newFact: string,
  collection: string
): Promise<string> {
  return await invoke<string>("user_edit_memory", {
    oldFactId,
    newFact,
    collection,
  });
}

/**
 * Soft deletes a memory fact (creates tombstone).
 */
export async function deleteMemoryFact(factId: string): Promise<void> {
  return await invoke<void>("user_delete_memory", { factId });
}
