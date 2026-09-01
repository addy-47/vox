import { invoke } from "@tauri-apps/api/core";

/**
 * Mirror of `SessionRow` (ipc/history.rs:42). History.tsx declares an
 * identical local interface — import from here during migration.
 */
export interface SessionRow {
  id: number;
  started_at: number;
  ended_at: number | null;
  turn_count: number;
  first_message: string | null;
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
