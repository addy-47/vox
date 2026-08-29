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
  | "Error";

/** Canonical Rust `InteractionOwner` enum (core/state.rs). */
export type InteractionOwner = "Assistant" | "Dictation";

/** Payload emitted on `state_changed` event. */
export type StateChangedPayload = InteractionState;

/** `transcript_partial` / `transcript_final` payload. */
export interface TranscriptPayload {
  turn_id: number;
  text: string;
  owner: InteractionOwner;
}

/** Mirror of `TelemetryData` emitted on `telemetry`. */
export interface TelemetryData {
  energy: number;
  vad_prob: number;
  low: number;
  mid: number;
  high: number;
}

/** `ptt_status` payload (services/ptt.rs). */
export interface PttStatusPayload {
  state: "IDLE" | "RECORDING" | "PROCESSING";
  session_id?: number;
}

/** `cpu_governor_warning` payload (lib.rs:322). */
export interface CpuGovernorWarningPayload {
  governor: string;
  optimal: boolean;
  advice: string;
}

/** `realtime_idle_warning` payload (ipc/pipeline.rs:1084). */
export interface RealtimeIdleWarningPayload {
  seconds_remaining: number;
}

/** `system_stats` payload (monitoring/system_monitor.rs:95). */
export interface SystemStatsPayload {
  system_cpu: number;
  system_ram_pct: number;
  vox_cpu: number;
  vox_ram_mb: number;
  threads: number;
  total_memory_gb: number;
  cpu_count: number;
}

/** `remote_setup_status` payload (ipc/settings.rs:952). */
export interface RemoteSetupStatusPayload {
  step: string;
  progress: number;
  log_line: string;
  error?: string;
}

/** Rust `SetupStep` enum (setup/model_manager.rs:14), used by `model_setup_status`. */
export type SetupStep =
  | "Idle"
  | "Downloading"
  | "Extracting"
  | "Verifying"
  | "Completed"
  | "Failed"
  | "Cancelled";

/** `model_setup_status` payload (setup/model_manager.rs:343). */
export interface ModelSetupStatusPayload {
  model_id: string;
  step: SetupStep;
  progress: number;
  bytes_downloaded: number;
  total_bytes: number;
  error: string | null;
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
 * Typed wrapper around Tauri `listen`. Returns a synchronous unlisten
 * function that is safe to call before the underlying listener resolves.
 */
export function on<T>(eventName: string, handler: (payload: T) => void): () => void {
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

  listen<T>(eventName, (event) => handler(event.payload))
    .then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })
    .catch((err) => {
      activeListeners.delete(cleanup);
      console.error(`[events] Failed to listen to "${eventName}":`, err);
    });

  return cleanup;
}

export function onModelSetupStatus(handler: (payload: ModelSetupStatusPayload) => void): () => void {
  return on("model_setup_status", handler);
}

export function onOptionalModelComplete(handler: (modelId: string) => void): () => void {
  return on("optional_model_complete", handler);
}

export function onRemoteSetupStatus(handler: (payload: RemoteSetupStatusPayload) => void): () => void {
  return on("remote_setup_status", handler);
}
