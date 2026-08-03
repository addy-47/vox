import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Rust `InteractionState` enum (core/state.rs:54). Drives mood sync and
 * all ambient visuals.
 */
export type InteractionState =
  | "Idle"
  | "Listening"
  | "UserSpeaking"
  | "Thinking"
  | "AssistantSpeaking"
  | "Interrupted"
  | "MaintainingContext";

/** Rust `InteractionOwner` enum (core/state.rs:11). */
export type InteractionOwner = "Tray" | "MainWindow" | "Ptt" | "Wizard";

/** Mirror of `TelemetryData` (core/state.rs:65), emitted on `telemetry`. */
export interface TelemetryData {
  energy: number;
  vad_prob: number;
  low: number;
  mid: number;
  high: number;
}

/** `transcript_partial` / `transcript_final` payload (services/pipeline.rs:801). */
export interface TranscriptPayload {
  text: string;
  turn_id: number;
  owner: InteractionOwner;
}

/** `ptt_status` payload (services/ptt.rs). */
export interface PttStatusPayload {
  state: "IDLE" | "RECORDING" | "PROCESSING";
  session_id?: number;
}

/** `speech_start` / `speech_end` payload (services/vad/actor.rs, re-emitted in ipc/pipeline.rs:529). */
export interface SpeechEventPayload {
  type: "speech_start" | "speech_end";
  session_id: number;
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
 * `playback_finished` payload — the per-turn latency report JSON
 * (services/pipeline.rs:1546; core/metrics.rs `latency_report`).
 */
export type PlaybackFinishedPayload = Record<string, unknown>;

/**
 * Typed wrapper around Tauri `listen`. Returns a synchronous unlisten
 * function that is safe to call before the underlying listener resolves.
 */
export function on<T>(eventName: string, handler: (payload: T) => void): () => void {
  let unlisten: UnlistenFn | null = null;
  let cancelled = false;
  listen<T>(eventName, (event) => handler(event.payload))
    .then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })
    .catch((err) => {
      console.error(`[events] Failed to listen to "${eventName}":`, err);
    });
  return () => {
    cancelled = true;
    if (unlisten) unlisten();
  };
}

export function onStateChanged(handler: (state: InteractionState) => void): () => void {
  return on("state_changed", handler);
}

export function onTranscriptPartial(handler: (payload: TranscriptPayload) => void): () => void {
  return on("transcript_partial", handler);
}

export function onTranscriptFinal(handler: (payload: TranscriptPayload) => void): () => void {
  return on("transcript_final", handler);
}

export function onLlmToken(handler: (token: string) => void): () => void {
  return on("llm_token", handler);
}

export function onPttStatus(handler: (payload: PttStatusPayload) => void): () => void {
  return on("ptt_status", handler);
}

export function onAudioEnergy(handler: (energy: number) => void): () => void {
  return on("audio_energy", handler);
}

export function onTelemetry(handler: (payload: TelemetryData) => void): () => void {
  return on("telemetry", handler);
}

export function onAutoSleepState(handler: (sleeping: boolean) => void): () => void {
  return on("auto_sleep_state", handler);
}

export function onCpuGovernorWarning(handler: (payload: CpuGovernorWarningPayload) => void): () => void {
  return on("cpu_governor_warning", handler);
}

export function onPlaybackFinished(handler: (payload: PlaybackFinishedPayload) => void): () => void {
  return on("playback_finished", handler);
}

export function onPipelineError(handler: (message: string) => void): () => void {
  return on("pipeline_error", handler);
}

export function onPipelinePaused(handler: () => void): () => void {
  return on("pipeline_paused", handler);
}

export function onPipelineResumed(handler: () => void): () => void {
  return on("pipeline_resumed", handler);
}

export function onRealtimeSessionStarted(handler: () => void): () => void {
  return on("realtime_session_started", handler);
}

export function onRealtimeSessionResumed(handler: () => void): () => void {
  return on("realtime_session_resumed", handler);
}

export function onRealtimeSessionEnded(handler: (reason: string) => void): () => void {
  return on("realtime_session_ended", handler);
}

export function onRealtimeIdleWarning(handler: (payload: RealtimeIdleWarningPayload) => void): () => void {
  return on("realtime_idle_warning", handler);
}

/** `mode_changed_main` / `mode_changed_tray` (dynamically named, ipc/tray.rs:250). */
export function onModeChanged(target: "main" | "tray", handler: (mode: string) => void): () => void {
  return on(`mode_changed_${target}`, handler);
}

/** Global `mode_changed` event (ipc/tray.rs:251). */
export function onModeChangedGlobal(handler: (mode: string) => void): () => void {
  return on("mode_changed", handler);
}

export function onThemeChanged(handler: (theme: string) => void): () => void {
  return on("theme-changed", handler);
}

export function onSettingsUpdated(handler: () => void): () => void {
  return on("settings-updated", handler);
}

export function onToggleHud(handler: () => void): () => void {
  return on("toggle_hud", handler);
}

export function onSpeechStart(handler: (payload: SpeechEventPayload) => void): () => void {
  return on("speech_start", handler);
}

export function onSpeechEnd(handler: (payload: SpeechEventPayload) => void): () => void {
  return on("speech_end", handler);
}

export function onModelSetupStatus(handler: (payload: ModelSetupStatusPayload) => void): () => void {
  return on("model_setup_status", handler);
}

export function onModelSetupComplete(handler: (completed: boolean) => void): () => void {
  return on("model_setup_complete", handler);
}

export function onModelSetupError(handler: (message: string) => void): () => void {
  return on("model_setup_error", handler);
}

export function onOptionalModelComplete(handler: (modelId: string) => void): () => void {
  return on("optional_model_complete", handler);
}

export function onRemoteSetupStatus(handler: (payload: RemoteSetupStatusPayload) => void): () => void {
  return on("remote_setup_status", handler);
}
