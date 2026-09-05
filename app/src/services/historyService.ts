import { invoke } from "@tauri-apps/api/core";
import { SESSION_COPY } from "@/data/sessionCopy";

/**
 * Mirror of `SessionRow` (ipc/history.rs:42). History.tsx declares an
 * identical local interface — import from here during migration.
 *
 * `title` / `last_activity` are forward-compatible session-continuation
 * fields: the backend adds them when title generation lands. They stay
 * optional so the current backend (first_message only) keeps working.
 */
export interface SessionRow {
  id: number;
  started_at: number;
  ended_at: number | null;
  turn_count: number;
  first_message: string | null;
  title?: string | null;
  last_activity?: number | null;
}

/**
 * Mirror of `TurnRow` (ipc/history.rs:51). History.tsx declares an
 * identical local interface — import from here during migration.
 */
export interface TurnRow {
  id: number;
  session_id: number;
  turn_id: number;
  user_text: string;
  assistant_text: string;
  stt_latency_ms: number | null;
  ttft_ms: number | null;
  created_at: number;
}

/** Ephemeral in-memory transcript history (tray buffer) (ipc/history.rs:8). */
export function getTranscriptHistory(): Promise<string[]> {
  return invoke("get_transcript_history");
}

/** Commit a session's full text to the ephemeral history buffer (ipc/history.rs:17). */
export function commitSessionToHistory(text: string): Promise<void> {
  return invoke("commit_session_to_history", { text });
}

/** All persisted sessions, most recent first (ipc/history.rs:64). */
export function getSessions(): Promise<SessionRow[]> {
  return invoke("get_sessions");
}

/** All turns for a session, oldest first (ipc/history.rs:99, arg `session_id`). */
export function getTurns(sessionId: number): Promise<TurnRow[]> {
  return invoke("get_turns", { sessionId });
}

/** Delete a session and its turns (CASCADE) (ipc/history.rs:138). */
export function deleteSession(id: number): Promise<void> {
  return invoke("delete_session", { id });
}

function isMissingCommandError(e: unknown): boolean {
  const msg = e instanceof Error ? e.message : String(e);
  return msg.includes("not found") || msg.includes("Unknown command");
}

/**
 * Make `sessionId` the active session on the backend, then return its
 * persisted turns oldest-first. Falls back to a local-only restore
 * (turns without backend selection) while the backend `select_session`
 * command is not implemented yet — real backend errors are rethrown.
 */
export async function selectSession(sessionId: number): Promise<TurnRow[]> {
  try {
    await invoke("select_session", { sessionId });
  } catch (e: unknown) {
    if (!isMissingCommandError(e)) throw e;
    console.warn("[history] select_session unavailable, restoring transcript locally");
  }
  return getTurns(sessionId);
}

/**
 * Create a fresh empty session on the backend. Falls back to a local-only
 * reset while the backend `start_new_conversation` command is not
 * implemented yet — real backend errors are rethrown.
 */
export async function startNewConversation(): Promise<void> {
  try {
    await invoke("start_new_conversation");
  } catch (e: unknown) {
    if (!isMissingCommandError(e)) throw e;
    console.warn("[history] start_new_conversation unavailable, resetting locally");
  }
}

/**
 * Display title for a session list entry: generated title once available,
 * first persisted user message as interim context, otherwise the neutral
 * untitled placeholder. Blank/whitespace titles never win.
 */
export function resolveSessionTitle(session: SessionRow): string {
  const generated = session.title?.trim();
  if (generated) return generated;
  const first = session.first_message?.trim();
  if (first) return first;
  return SESSION_COPY.untitledSession;
}

/** Last-activity timestamp driving newest-first ordering (spec §A.3). */
export function sessionLastActivity(session: SessionRow): number {
  return session.last_activity ?? session.started_at;
}

/** Sort sessions newest-first by last activity without mutating the input. */
export function sortSessionsNewestFirst(sessions: SessionRow[]): SessionRow[] {
  return [...sessions].sort((a, b) => sessionLastActivity(b) - sessionLastActivity(a));
}

/** Human recency for a session list entry ("5m ago", "Yesterday", date). */
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
