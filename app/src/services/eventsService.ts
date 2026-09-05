import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  | "Sleeping";

/** Canonical Rust `InteractionOwner` enum (core/state.rs). */
export type InteractionOwner = "Assistant" | "Dictation";

/** Payload emitted on `state_changed` event. */
export interface StateChangedPayload {
  owner: InteractionOwner;
  state: string;
  turn_id: number;
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

/** `show_toast` payload (core/events.rs: ToastPayload). */
export type ToastLevel = "success" | "warning" | "error" | "info";
export interface ToastPayload {
  title: string;
  message: string;
  level: ToastLevel;
  duration_ms?: number;
}

export interface NotificationRecord {
  id: string;
  category: string;
  title: string;
  message: string;
  status: string;
  session_id?: number | null;
  metadata: string;
  is_read: boolean;
  created_at: number;
}

export interface NotificationDismissedPayload {
  id: string;
}

/** `session_title_updated` payload — backend title consumer persisted a title. */
export interface SessionTitleUpdatedPayload {
  session_id: number;
  title: string;
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
  "settings-updated": void;
  toggle_tray: void;
  show_toast: ToastPayload;
  notification_created: NotificationRecord;
  notification_updated: NotificationRecord;
  notification_dismissed: NotificationDismissedPayload;
  notifications_marked_read: void;
  session_title_updated: SessionTitleUpdatedPayload;
  sessions_changed: void;
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

  listen<IpcEventMap[K]>(eventName as string, (event) => handler(event.payload))
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

export function onShowToast(handler: (payload: ToastPayload) => void): () => void {
  return on("show_toast", handler);
}

export function onNotificationCreated(handler: (payload: NotificationRecord) => void): () => void {
  return on("notification_created", handler);
}

export function onNotificationUpdated(handler: (payload: NotificationRecord) => void): () => void {
  return on("notification_updated", handler);
}

export function onNotificationDismissed(handler: (payload: NotificationDismissedPayload) => void): () => void {
  return on("notification_dismissed", handler);
}

export function onNotificationsMarkedRead(handler: () => void): () => void {
  return on("notifications_marked_read", handler);
}

export function onSessionTitleUpdated(
  handler: (payload: SessionTitleUpdatedPayload) => void
): () => void {
  return on("session_title_updated", handler);
}

export function onSessionsChanged(handler: () => void): () => void {
  return on("sessions_changed", handler);
}
