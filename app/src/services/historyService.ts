import { invoke } from "@tauri-apps/api/core";
import { SESSION_COPY } from "@/data/sessionCopy";

/**
 * Mirror of `SessionRow` (persistence/sessions.rs).
 * v2 schema: replaces legacy `started_at`/`ended_at`/`last_activity` with
 * `project_id`, `is_pinned`, `deleted_at`, and `updated_at`.
 */
export interface SessionRow {
  id: number;
  project_id: string;
  title: string | null;
  is_pinned: boolean;
  deleted_at: number | null;
  created_at: number;
  updated_at: number;
  turn_count: number;
  first_message: string | null;
}

/**
 * Mirror of `TurnRow` (persistence/sessions.rs).
 * v2 schema: removes latency columns (`stt_latency_ms`, `ttft_ms`).
 */
export interface TurnRow {
  id: number;
  session_id: number;
  turn_id: number;
  user_text: string;
  assistant_text: string;
  created_at: number;
}

/** Result payload returned by `continue_session` (ipc/history.rs). */
export interface ContinueSessionResult {
  session: SessionRow;
  turns: TurnRow[];
}

/** Optional fields for `update_session`. All are nullable to support partial updates. */
export interface SessionUpdate {
  title?: string | null;
  isPinned?: boolean | null;
  projectId?: string | null;
}

/** Ephemeral in-memory transcript history (tray buffer). */
export function getTranscriptHistory(): Promise<string[]> {
  return invoke("get_transcript_history");
}

// Lifecycle functions moved to pipelineService to match ipc/pipeline.rs domain
export { createSession, continueSession } from "./pipelineService";

const sessionsInFlight = new Map<string, Promise<SessionRow[]>>();

/** Returns active sessions optionally filtered by project, pinned-first then newest. */
export function getSessions(projectId?: string): Promise<SessionRow[]> {
  const key = projectId ?? "__all__";
  const existing = sessionsInFlight.get(key);
  if (existing) {
    return existing;
  }
  const promise = invoke<SessionRow[]>("get_sessions", { projectId: projectId ?? null })
    .finally(() => {
      sessionsInFlight.delete(key);
    });
  sessionsInFlight.set(key, promise);
  return promise;
}

const turnsInFlight = new Map<number, Promise<TurnRow[]>>();

/** Returns all turns for a session, oldest first. */
export function getTurns(sessionId: number): Promise<TurnRow[]> {
  const existing = turnsInFlight.get(sessionId);
  if (existing) {
    return existing;
  }
  const promise = invoke<TurnRow[]>("get_turns", { sessionId })
    .finally(() => {
      turnsInFlight.delete(sessionId);
    });
  turnsInFlight.set(sessionId, promise);
  return promise;
}

/**
 * Applies partial metadata updates to a session.
 * Emits `SessionsChanged` on success.
 */
export function updateSession(sessionId: number, updates: SessionUpdate): Promise<void> {
  return invoke("update_session", {
    sessionId,
    title: updates.title ?? null,
    isPinned: updates.isPinned ?? null,
    projectId: updates.projectId ?? null,
  });
}

/**
 * Deletes a session. `hard = true` permanently purges with CASCADE; default is soft delete.
 * Emits `SessionsChanged` on success.
 */
export function deleteSession(sessionId: number, hard = false): Promise<void> {
  return invoke("delete_session", { sessionId, hard });
}

// ─── Presentation Utilities ──────────────────────────────────────────────────

/**
 * Display title for a session list entry: generated title once available,
 * first persisted user message as interim context, otherwise neutral placeholder.
 */
export function resolveSessionTitle(session: SessionRow): string {
  const generated = session.title?.trim();
  if (generated) return generated;
  const first = session.first_message?.trim();
  if (first) return first;
  return SESSION_COPY.untitledSession;
}

/** Timestamp driving newest-first ordering: `updated_at` reflects the last persisted turn only (metadata edits never bump it). */
export function sessionLastActivity(session: SessionRow): number {
  return session.updated_at;
}

/** Sorts sessions pinned-first then newest by `updated_at` without mutating the input. */
export function sortSessionsNewestFirst(sessions: SessionRow[] | null | undefined): SessionRow[] {
  if (!Array.isArray(sessions)) return [];
  return [...sessions].sort((a, b) => {
    if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
    return sessionLastActivity(b) - sessionLastActivity(a);
  });
}

export { formatSessionRecency, formatDateShort, formatDateTime } from "@/shared/lib/dateTime";
