import { invoke } from "@tauri-apps/api/core";

export interface MemoryNodeTopology {
  id: string;
  collection: string;
  is_superseded: boolean;
  created_at: number;
  fact?: string;
}

export interface MemoryEdgeTopology {
  id: number;
  from_id: string;
  to_id: string;
  relation: string;
  created_at: number;
}

export interface MemoryGraphPayload {
  version: number;
  nodes: MemoryNodeTopology[];
  edges: MemoryEdgeTopology[];
}

export interface MemoryFactDetail {
  id: string;
  collection: string;
  fact: string;
  source: string;
  session_id: string;
  created_at: number;
  is_superseded: boolean;
  incoming_relations: MemoryEdgeTopology[];
  outgoing_relations: MemoryEdgeTopology[];
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
  failed_items?: MemoryQueueItem[];
  recent_items: MemoryQueueItem[];
}

export interface MemoryConflict {
  fact_a: MemoryNodeTopology;
  fact_b: MemoryNodeTopology;
}

/**
 * Reads current atomic graph version.
 */
export async function getGraphVersion(): Promise<number> {
  try {
    return await invoke<number>("get_graph_version");
  } catch (err) {
    console.error("Failed to get graph version:", err);
    return 0;
  }
}

/**
 * Fetches lightweight node topology and relation edges.
 */
export async function getMemoryGraphTopology(includeInactive = false): Promise<MemoryGraphPayload | null> {
  try {
    return await invoke<MemoryGraphPayload>("get_memory_graph_topology", {
      filter: { include_inactive: includeInactive },
    });
  } catch (err) {
    console.error("Failed to fetch memory graph topology:", err);
    return null;
  }
}

/**
 * Lazy loads full details for a single memory fact node.
 */
export async function getMemoryFactDetail(factId: string): Promise<MemoryFactDetail | null> {
  try {
    return await invoke<MemoryFactDetail>("get_memory_fact_detail", { factId });
  } catch (err) {
    console.error("Failed to fetch memory fact detail:", err);
    return null;
  }
}

/**
 * Edits raw fact text in-place (synchronizing vector embeddings).
 */
export async function editFactContent(factId: string, newContent: string): Promise<void> {
  await invoke<void>("edit_fact_content", { factId, newContent });
}

/**
 * Reassigns fact to a new collection category via pipeline staging.
 */
export async function reassignFactCollection(factId: string, newCollection: string): Promise<void> {
  await invoke<void>("reassign_fact_collection", { factId, newCollection });
}

/**
 * Soft deletes a memory fact.
 */
export async function softDeleteFact(factId: string): Promise<void> {
  await invoke<void>("soft_delete_fact", { factId });
}

/**
 * Toggles background pipeline processing pause state.
 */
export async function togglePipelineProcessing(paused?: boolean): Promise<boolean> {
  return await invoke<boolean>("toggle_pipeline_processing", { paused });
}

/**
 * Resets all failed queue items back to staged_pending.
 */
export async function retryFailedQueue(): Promise<number> {
  return await invoke<number>("retry_failed_queue");
}

/**
 * Resets specific failed queue items back to staged_pending.
 */
export async function retryFailedQueueItems(itemIds: number[]): Promise<number> {
  return await invoke<number>("retry_failed_queue_items", { itemIds });
}

/**
 * Discovers unresolved memory conflicts.
 */
export async function getUnresolvedConflicts(): Promise<MemoryConflict[]> {
  try {
    return await invoke<MemoryConflict[]>("get_unresolved_conflicts");
  } catch (err) {
    console.error("Failed to fetch unresolved conflicts:", err);
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
