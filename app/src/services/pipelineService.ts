import { invoke } from "@tauri-apps/api/core";

// ─── Engine & Pipeline Types ───────────────────────────────────────────────

export type InteractionOwner = "Assistant" | "Dictation";

export interface RuntimeSnapshot {
  pipeline_state: string;
  current_turn_id: number;
  conversation_id: number;
  playback_active: boolean;
  tts_generating: boolean;
  system_cpu_usage: number;
  system_ram_mb: number;
  vox_cpu_usage: number;
  vox_ram_mb: number;
  total_ram_mb: number;
  cpu_cores: number;
  vad_energy: number;
  vad_probability: number;
  stt_latency_ms: number | null;
  ttft_ms: number | null;
  total_voice_latency_ms: number | null;
  persistence_queue_depth: number;
  dropped_persistence_events: number;
  playback_buffer_samples: number;
  playback_underruns: number;
  active_owner: string;
  active_threads: number;
  tts_rtf: number | null;
  playback_start_ms: number | null;
  persistence_writes_per_sec: number;
  is_db_healthy: boolean;
  is_llm_loaded: boolean;
  llm_provider_kind: string;
  is_tts_loaded: boolean;
  is_stt_loaded: boolean;
  is_vad_loaded: boolean;
  is_embedder_loaded: boolean;
  is_query_classifier_loaded: boolean;
  is_intra_edge_classifier_loaded: boolean;
  is_inter_edge_classifier_loaded: boolean;
  is_translit_loaded: boolean;
  is_sleeping: boolean;
  is_engaged: boolean;
  cpu_governor: string;
  cpu_governor_optimal: boolean;
  timestamp_ms: number;
}

/** RuntimeSnapshot with a local performance.now() timestamp for sparkline age calc. */
export type LocalSnapshot = RuntimeSnapshot & { localTime: number };

export interface RealtimeSessionCache {
  has_session: boolean;
  provider: string;
  expires_in_seconds: number;
  model: string;
}

// ─── Engine Commands ───────────────────────────────────────────────────────

export function stopEngine(): Promise<void> {
  return invoke("stop_engine");
}

export function launchEngine(): Promise<void> {
  return invoke("launch_engine");
}

export function startSession(): Promise<void> {
  return invoke("start_session");
}

export function endSession(): Promise<void> {
  return invoke("end_session");
}

export function pauseSession(): Promise<void> {
  return invoke("pause_session");
}

export function resumeSession(): Promise<void> {
  return invoke("resume_session");
}

export function pttStart(): Promise<void> {
  return invoke("ptt_start");
}

export function pttStop(): Promise<void> {
  return invoke("ptt_stop");
}

export function pttCancel(): Promise<void> {
  return invoke("ptt_cancel");
}

export function testClip(clipId: string): Promise<void> {
  return invoke("test_clip", { clipId });
}

export function testClipCancel(): Promise<void> {
  return invoke("test_clip_cancel");
}

export function getRuntimeSnapshot(): Promise<RuntimeSnapshot | null> {
  return invoke("get_runtime_snapshot");
}

export function getRuntimeHistory(): Promise<RuntimeSnapshot[]> {
  return invoke("get_runtime_history");
}

export function clearRuntimeHistory(): Promise<void> {
  return invoke("clear_runtime_history");
}

export function getRealtimeSessionCache(): Promise<RealtimeSessionCache> {
  return invoke("get_realtime_session_cache");
}

// ─── Voice Commands & Types ────────────────────────────────────────────────

export interface VoiceEntryDto {
  id: string;
  name: string;
  source_kind: string;
  has_preview: boolean;
  created_at: number;
}

export interface EdgeTtsVoiceDto {
  name: string;
  short_name: string;
  gender: string;
  locale: string;
  friendly_name: string;
}

export function startBackendRecording(): Promise<void> {
  return invoke("start_backend_recording");
}

export function stopBackendRecording(): Promise<[number[], number]> {
  return invoke("stop_backend_recording");
}

export function listVoices(): Promise<VoiceEntryDto[]> {
  return invoke("list_voices");
}

export function addVoiceFromFile(name: string, filePath: string): Promise<VoiceEntryDto> {
  return invoke("add_voice_from_file", { name, filePath });
}

export function addVoiceFromRecording(name: string, pcmF32: number[], sampleRate: number): Promise<VoiceEntryDto> {
  return invoke("add_voice_from_recording", { name, pcmF32, sampleRate });
}

export function deleteVoice(id: string): Promise<void> {
  return invoke("delete_voice", { id });
}

export function fetchEdgeTtsVoices(): Promise<EdgeTtsVoiceDto[]> {
  return invoke("fetch_edge_tts_voices");
}

// ─── Remote Deployment Commands & Types ────────────────────────────────────

export interface RemoteServerConfig {
  connectionString: string;
  sshPort: number | null;
  identityKeyPath: string | null;
  remotePath: string;
  serverPort: number;
}

export function setupRemoteServer(config: RemoteServerConfig): Promise<void> {
  return invoke("setup_remote_server", { ...config });
}
