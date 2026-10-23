import { invoke } from "./transport";
import { onMemoryIngestionUpdated } from "./eventsService";

/**
 * Mirror of `PersonalMemoryRecord` (persistence/personal_memory.rs).
 * Structured semantic model with rendered markdown cache.
 */
export interface PersonalMemoryRecord {
  id: number;
  project_id: string | null;
  /** Canonical JSON representation of PersonalMemory (`{"sections": [...]}`). */
  content: string;
  /** Markdown rendered from the canonical semantic model. */
  markdown: string;
  /** Monotonic version counter — must be passed back in save to detect concurrent edits. */
  version: number;
  is_active?: number;
  last_consolidated_at: number;
  updated_at: number;
}

export interface MemoryBlock {
  id: string;
  text: string;
}

export interface MemorySection {
  id: string;
  title: string;
  blocks: MemoryBlock[];
}

export interface SemanticPersonalMemory {
  sections: MemorySection[];
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
 * Saves direct manual edits to the rendered Markdown document.
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

export type ConfirmationReason = "compaction_in_progress" | "pending_queue_items";

export type ConsolidateOutcome =
  | { status: "completed"; record: PersonalMemoryRecord }
  | { status: "confirmation_required"; reason: ConfirmationReason; pending_count: number };

/**
 * Merges active personal observations or applies directive comments to consolidate the
 * personal memory document via LLM. Emits `PersonalMemoryUpdated` on success.
 */
export function consolidatePersonalMemory(
  comments?: string[],
  projectId?: string,
  forced?: boolean,
): Promise<ConsolidateOutcome> {
  return invoke("consolidate_personal_memory", {
    comments: comments ?? null,
    projectId: projectId ?? null,
    forced: forced ?? null,
  });
}

/**
 * Reformats and clarifies the existing personal memory document using LLM on demand.
 * Operates on current semantic memory, not on raw observations.
 */
export function regeneratePersonalMemory(projectId?: string): Promise<PersonalMemoryRecord> {
  return invoke("regenerate_personal_memory", { projectId: projectId ?? null });
}

/**
 * Projected semantic revision view from `personal_memory_revisions`.
 */
export interface MemoryRevisionView {
  id: string;
  /** `create_section` | `create_block` | `update_block` | `delete_block`. */
  op: string;
  /** The persistent `sec_*` or `blk_*` ID this operation targets, empty for `create_section`. */
  target_id: string;
  status: string;
  created_at: number;
  /** Human-readable one-line description, resolved against the active semantic model. */
  preview: string;
  /** Full JSON payload of the ResolvedOp. */
  content: string;
  /** Original text of target block prior to update or deletion. */
  old_text?: string | null;
}

export interface RevisionDecision {
  id: string;
  action: "accept" | "reject";
}

export interface ResolveRevisionsRequest {
  projectId?: string | null;
  decisions: RevisionDecision[];
}

/**
 * Lists all pending semantic memory revisions awaiting review.
 */
export function getMemoryRevisions(projectId?: string): Promise<MemoryRevisionView[]> {
  return invoke("get_memory_revisions", { projectId: projectId ?? null });
}

/**
 * Resolves a batch of pending personal memory revisions in a single atomic transaction.
 */
export function resolveMemoryRevisions(
  request: ResolveRevisionsRequest,
): Promise<PersonalMemoryRecord> {
  return invoke("resolve_memory_revisions", { request });
}

/**
 * Mirror of `ObservationRecord` (persistence/facts.rs).
 * Represents a single active memory observation extracted from a session.
 */
export interface ObservationRecord {
  id: string;
  session_id: number | null;
  compaction_id: number;
  observation_type: string;
  /** Normalized compatibility alias */
  fact_type: string;
  text: string;
  status: string;
  created_at: number;
  updated_at: number;
}

/**
 * Returns observations from `memory_facts` across all or filtered statuses (`active`, `integrated`, etc.),
 * with optional limit and offset pagination.
 */
export async function getObservations(
  projectId?: string,
  status?: string,
  limit?: number,
  offset?: number,
  observationType?: string
): Promise<ObservationRecord[]> {
  const records = await invoke<Array<Omit<ObservationRecord, "fact_type"> & { fact_type?: string }>>(
    "get_observations",
    {
      projectId: projectId ?? null,
      status: status ?? null,
      limit: limit ?? null,
      offset: offset ?? null,
      observationType: observationType ?? null,
    }
  );
  return records.map((r) => ({
    ...r,
    fact_type: r.observation_type,
  }));
}

/**
 * Returns all `status = 'active'` observations for memory graph visualization.
 *
 * Stale-while-revalidate read: the Memory page remounts on every route visit,
 * so a warm cache serves the last-known snapshot synchronously (zero IPC on
 * warm navigation) while a deduped background revalidate updates the cache.
 * The cache is invalidated by the ingestion event below — the only backend
 * writer of active observations — and by `{ force: true }` for local
 * mutation paths.
 */
let activeObservationsCache: ObservationRecord[] | null = null;
let activeObservationsInFlight: Promise<ObservationRecord[]> | null = null;

function fetchActiveObservations(): Promise<ObservationRecord[]> {
  if (activeObservationsInFlight) return activeObservationsInFlight;
  activeObservationsInFlight = getObservations(undefined, "active").finally(() => {
    activeObservationsInFlight = null;
  });
  return activeObservationsInFlight;
}

export async function getActiveObservations(opts?: { force?: boolean }): Promise<ObservationRecord[]> {
  if (opts?.force || !activeObservationsCache) {
    const data = await fetchActiveObservations();
    activeObservationsCache = data;
    return data;
  }
  void fetchActiveObservations()
    .then((fresh) => {
      activeObservationsCache = fresh;
    })
    .catch(() => {
      // Background revalidate is best-effort; the stale snapshot stays served.
    });
  return activeObservationsCache;
}

// Compaction/ingestion is the only backend writer of active observations;
// hook the canonical event once at module scope so the cache never outlives
// the last backend mutation. eventsService imports memoryService type-only,
// so this direction introduces no runtime import cycle.
onMemoryIngestionUpdated(() => {
  activeObservationsCache = null;
});

export interface IngestionStats {
  total: number;
  pending: number;
  processing: number;
  completed: number;
  failed: number;
  unconsolidated_personal_count: number;
}

/**
 * Returns aggregate counts of memory ingestion queue and active facts.
 */
export function getIngestionStats(): Promise<IngestionStats> {
  return invoke("get_ingestion_stats");
}

/* backend-gap: Unified Hybrid Vector RRF Search
 * Currently, candidate extraction queries keyword matches and vector similarity matches
 * independently, executing reciprocal rank fusion (rrfScore k=60) on the client thread.
 * Rust backend contract must expose:
 *   search_memory({ query: string, mode: "hybrid" | "vector" | "keyword", limit: number })
 * returning pre-ranked results directly from Turso DB.
 */


