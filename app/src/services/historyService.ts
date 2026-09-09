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

/**
 * Initializes a fresh conversation session on the backend, resetting working memory.
 * Emits `SessionsChanged` on success.
 */
export function createSession(projectId?: string): Promise<SessionRow> {
  return invoke("create_session", { projectId: projectId ?? null });
}

/**
 * Restores a past session into working memory and returns its full context.
 * Emits `SessionsChanged` on success.
 */
export function continueSession(sessionId: number): Promise<ContinueSessionResult> {
  return invoke("continue_session", { sessionId });
}

/** Returns active sessions optionally filtered by project, pinned-first then newest. */
export function getSessions(projectId?: string): Promise<SessionRow[]> {
  return invoke("get_sessions", { projectId: projectId ?? null });
}

/** Returns all turns for a session, oldest first. */
export function getTurns(sessionId: number): Promise<TurnRow[]> {
  return invoke("get_turns", { sessionId });
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

/** Timestamp driving newest-first ordering: `updated_at` is bumped on every mutation. */
export function sessionLastActivity(session: SessionRow): number {
  return session.updated_at;
}

/** Sorts sessions pinned-first then newest by `updated_at` without mutating the input. */
export function sortSessionsNewestFirst(sessions: SessionRow[]): SessionRow[] {
  return [...sessions].sort((a, b) => {
    if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
    return sessionLastActivity(b) - sessionLastActivity(a);
  });
}

/** Human recency label for a session list entry ("5m ago", "Yesterday", date). */
export function formatSessionRecency(timestampMs: number, nowMs: number = Date.now()): string {
  const diffMs = Math.max(0, nowMs - timestampMs);
  const minutes = Math.floor(diffMs / 60000);
  if (minutes < 1) return SESSION_COPY.recency.justNow;
  if (minutes < 60) return SESSION_COPY.recency.minutesAgo.replace("{n}", String(minutes));
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return SESSION_COPY.recency.hoursAgo.replace("{n}", String(hours));
  if (diffMs < 48 * 3600000) return SESSION_COPY.recency.yesterday;
  return new Date(timestampMs).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatDateShort(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatDateTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
