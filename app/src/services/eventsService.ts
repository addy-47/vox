import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PersonalMemoryRecord } from "./memoryService";

/**
 * Canonical Rust `InteractionState` enum (core/state.rs).
 * Drives mood sync, visualizers, and UI state indicators.
 */
export type InteractionState =
  | "Idle"
  | "Ready"
  | "Listening"
  | "Thinking"
  | "Speaking"
  | "Paused"
  | "Error"
  | "Sleeping"
  | "Working";

export const VALID_INTERACTION_STATES = new Set<InteractionState>([
  "Idle",
  "Ready",
  "Listening",
  "Thinking",
  "Speaking",
  "Paused",
  "Error",
  "Sleeping",
  "Working",
]);

/** Canonical Rust `InteractionOwner` enum (core/state.rs). */
export type InteractionOwner = "Assistant" | "Dictation";

export type ActivityKind = "tool" | "compaction";

export interface ActivityEnvelope {
  kind: ActivityKind;
  name: string;
  call_id?: string | null;
}

/** Payload emitted on `state_changed` event. */
export interface StateChangedPayload {
  owner: InteractionOwner;
  state: string;
  turn_id: number;
  activity?: ActivityEnvelope | null;
}

/** `transcript_partial` / `transcript_final` payload. */
export interface TranscriptPayload {
  turn_id: number;
  text: string;
  owner?: InteractionOwner;
}

/** `llm_token` streaming delta payload. */
export interface LlmTokenPayload {
  turn_id: number;
  token: string;
}

/** Mirror of `TelemetryData` emitted on `telemetry`. */
export interface TelemetryData {
  energy: number;
  vad_prob: number;
  low: number;
  mid: number;
  high: number;
}

/** `system_stats` payload (monitoring/system_monitor.rs). */
export interface SystemStatsPayload {
  system_cpu: number;
  system_ram_pct: number;
  vox_cpu: number;
  vox_ram_mb: number;
  threads: number;
  total_memory_gb: number;
  cpu_count: number;
}

/** Rust `SetupStep` enum (setup/model_manager.rs:14), used by `model_progress`. */
export type SetupStep =
  | "idle"
  | "downloading"
  | "extracting"
  | "verifying"
  | "completed"
  | "failed"
  | "cancelled"
  | "Idle"
  | "Downloading"
  | "Extracting"
  | "Verifying"
  | "Completed"
  | "Failed"
  | "Cancelled";

/** `model_progress` payload (setup/model_manager.rs & health.rs). */
export interface ModelProgressPayload {
  model_id: string;
  step: SetupStep;
  progress: number;
  bytes_downloaded: number;
  total_bytes: number;
  error: string | null;
}

/** Universal visual priority severity (core/events.rs: Severity). */
export type Severity = "info" | "warning" | "critical";

export interface NotificationRecord {
  id: string;
  group_key: string;
  category: string;
  severity: Severity;
  action_type: string;
  action_payload: string;
  title: string;
  message: string;
  status: string;
  session_id?: number | null;
  metadata: string;
  created_at: number;
  updated_at: number;
}

/** `turn_metrics` payload emitted on turn start/finish. */
export interface TurnMetricsPayload {
  turn_id: number;
  ttft_ms: number;
  ttfa_ms: number;
  total_voice_latency_ms: number;
  context_tokens_used: number;
  context_window: number;
}

/**
 * Canonical IPC Event Map mirroring Rust `IpcEvent` registry in `core/events.rs`.
 */
export interface IpcEventMap {
  state_changed: StateChangedPayload;
  transcript_partial: TranscriptPayload;
  transcript_final: TranscriptPayload;
  llm_token: LlmTokenPayload;
  model_progress: ModelProgressPayload;
  telemetry: TelemetryData;
  system_stats: SystemStatsPayload;
  turn_metrics: TurnMetricsPayload;
  "settings-updated": void;
  toggle_tray: void;
  notification_created: NotificationRecord;
  notification_updated: NotificationRecord;
  session_title_updated?: never; // removed in v2 — title changes surface via sessions_changed
  sessions_changed: void;
  personal_memory_updated: PersonalMemoryRecord;
  memory_ingestion_updated: void;
}

/**
 * Active listener registry to guarantee synchronous teardown before webview unloads/reloads.
 */
const activeListeners = new Set<() => void>();

if (typeof window !== "undefined") {
  const cleanupAll = () => {
    activeListeners.forEach((cleanup) => {
      try {
        cleanup();
      } catch (err) {
        console.warn("[events] Error during beforeunload cleanup:", err);
      }
    });
    activeListeners.clear();
  };

  window.addEventListener("beforeunload", cleanupAll, { capture: true });
  window.addEventListener("pagehide", cleanupAll, { capture: true });
}

/**
 * Strongly-typed wrapper around Tauri `listen`, generic over canonical `IpcEventMap`.
 * Returns a synchronous unlisten function safe to call before the listener resolves.
 */
export function on<K extends keyof IpcEventMap>(
  eventName: K,
  handler: (payload: IpcEventMap[K]) => void
): () => void {
  let unlisten: UnlistenFn | null = null;
  let cancelled = false;

  const cleanup = () => {
    cancelled = true;
    activeListeners.delete(cleanup);
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  };

  activeListeners.add(cleanup);

  listen<IpcEventMap[K]>(eventName, (event) => handler(event.payload))
    .then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })
    .catch((err) => {
      activeListeners.delete(cleanup);
      console.error(`[events] Failed to listen to "${String(eventName)}":`, err);
    });

  return cleanup;
}


export function onStateChanged(handler: (payload: StateChangedPayload) => void): () => void {
  return on("state_changed", handler);
}

export function onTranscriptPartial(handler: (payload: TranscriptPayload) => void): () => void {
  return on("transcript_partial", handler);
}

export function onTranscriptFinal(handler: (payload: TranscriptPayload) => void): () => void {
  return on("transcript_final", handler);
}

export function onLlmToken(handler: (payload: LlmTokenPayload) => void): () => void {
  return on("llm_token", handler);
}

export function onModelProgress(handler: (payload: ModelProgressPayload) => void): () => void {
  return on("model_progress", handler);
}

export function onToggleTray(handler: () => void): () => void {
  return on("toggle_tray", handler);
}

export function onTelemetry(handler: (payload: TelemetryData) => void): () => void {
  return on("telemetry", handler);
}

export function onSystemStats(handler: (payload: SystemStatsPayload) => void): () => void {
  return on("system_stats", handler);
}

export function onSettingsUpdated(handler: () => void): () => void {
  return on("settings-updated", handler);
}

export function onNotificationCreated(handler: (payload: NotificationRecord) => void): () => void {
  return on("notification_created", handler);
}

export function onNotificationUpdated(handler: (payload: NotificationRecord) => void): () => void {
  return on("notification_updated", handler);
}

// ── sessions_changed coalescing ─────────────────────────────────────────
// The backend can emit sessions_changed in bursts (e.g. once per persisted
// turn — title updates surface here since session_title_updated was removed
// in v2). Two components subscribe independently (useSessionPanel.refresh =
// getSessions+getProjects, ActiveSessionHeader = snapshot+getSessions+
// getProjects), so one burst used to cost 4+ full-list IPC queries plus a
// 60ms artificial delay. This single trailing timer fans one notification out
// to all subscribers per burst instead of one per event.
// REVERT: delete this block and call handler() directly in onSessionsChanged.
// ─────────────────────────────────────────────────────────────────────────
const SESSIONS_CHANGED_COALESCE_MS = 250;
const SESSIONS_CHANGED_TEARDOWN_MS = 1000;
const sessionsChangedHandlers = new Set<() => void>();
let sessionsChangedTimer: ReturnType<typeof setTimeout> | null = null;
let sessionsChangedUnlisten: (() => void) | null = null;
let sessionsChangedTeardownTimer: ReturnType<typeof setTimeout> | null = null;

function flushSessionsChanged(): void {
  if (sessionsChangedTimer !== null) {
    clearTimeout(sessionsChangedTimer);
    sessionsChangedTimer = null;
  }
  sessionsChangedHandlers.forEach((h) => {
    try {
      h();
    } catch (e) {
      console.error("[Events] sessions_changed handler failed:", e);
    }
  });
}

function ensureSessionsChangedListener(): void {
  if (sessionsChangedTeardownTimer !== null) {
    clearTimeout(sessionsChangedTeardownTimer);
    sessionsChangedTeardownTimer = null;
  }
  if (sessionsChangedUnlisten !== null) return;
  sessionsChangedUnlisten = on("sessions_changed", () => {
    console.info("[Events] sessions_changed received");
    if (sessionsChangedTimer !== null) clearTimeout(sessionsChangedTimer);
    sessionsChangedTimer = setTimeout(flushSessionsChanged, SESSIONS_CHANGED_COALESCE_MS);
  });
}

export function onSessionsChanged(handler: () => void): () => void {
  ensureSessionsChangedListener();
  sessionsChangedHandlers.add(handler);
  let released = false;
  return () => {
    if (released) return;
    released = true;
    sessionsChangedHandlers.delete(handler);
    if (sessionsChangedHandlers.size > 0) return;
    // Last subscriber left. Leave any in-flight burst queued rather than
    // discarding it, and hold the listener open briefly so a same-tick
    // re-subscribe reuses it instead of racing a fresh registration.
    if (sessionsChangedTeardownTimer !== null) {
      clearTimeout(sessionsChangedTeardownTimer);
    }
    sessionsChangedTeardownTimer = setTimeout(() => {
      sessionsChangedTeardownTimer = null;
      if (sessionsChangedHandlers.size > 0) return;
      if (sessionsChangedUnlisten !== null) {
        sessionsChangedUnlisten();
        sessionsChangedUnlisten = null;
      }
    }, SESSIONS_CHANGED_TEARDOWN_MS);
  };
}

export function onTurnMetrics(handler: (payload: TurnMetricsPayload) => void): () => void {
  return on("turn_metrics", handler);
}

export function onMemoryIngestionUpdated(handler: () => void): () => void {
  return on("memory_ingestion_updated", handler);
}
