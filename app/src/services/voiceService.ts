import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export type UnifiedVoiceGender = "female" | "male" | "neutral";
export type UnifiedVoiceSource =
  | "bundled"
  | "custom"
  | "remote"
  | "pre_baked"
  | "zipvoice_pack"
  | "edge";

export interface UnifiedVoiceDto {
  id: string;
  provider_id: string;
  name: string;
  description: string | null;
  gender: UnifiedVoiceGender;
  locale: string;
  accent: string | null;
  tags: string[];
  sample_text: string;
  source_kind: UnifiedVoiceSource | string;
  is_favorite: boolean;
  last_used_at: number | null;
  has_preview: boolean;
}

export type VoiceEntryDto = UnifiedVoiceDto;

export interface VoicePreviewResult {
  cached: boolean;
  path: string;
  duration_ms: number;
  audio_base64?: string | null;
}

/** Lists unified voices filtered by active or specified TTS provider. */
export function listVoices(provider?: string): Promise<UnifiedVoiceDto[]> {
  return invoke("list_voices", { provider });
}

export interface VoiceScopeDto {
  voice_provider: string;
  can_clone: boolean;
  cloud_sync: boolean;
}

/** Resolves an active TTS engine id to its voice catalog scope (backend-owned mapping). */
export function getVoiceScope(engine?: string): Promise<VoiceScopeDto> {
  return invoke("get_voice_scope", { engine });
}

/** Background non-blocking sync of remote Edge TTS voices into the database. */
export function syncRemoteVoices(): Promise<number> {
  return invoke("sync_remote_voices");
}

/** Synthesizes or retrieves cached audio preview for a voice and prompt. */
export function synthesizeVoicePreview(
  voiceId: string,
  prompt?: string,
  engine?: string
): Promise<VoicePreviewResult> {
  return invoke("synthesize_voice_preview", { voiceId, prompt, engine });
}

/** Toggles favorite state for a voice. Returns the new favorite boolean value. */
export function toggleVoiceFavorite(voiceId: string): Promise<boolean> {
  return invoke("toggle_voice_favorite", { voiceId });
}

/** Records usage timestamp for recents sorting. */
export function recordVoiceUsed(voiceId: string): Promise<void> {
  return invoke("record_voice_used", { voiceId });
}

/** Sets active voice in runtime and updates persisted settings. */
export function setActiveVoice(voiceId: string): Promise<void> {
  return invoke("set_active_voice", { voiceId });
}

/** Starts microphone recording in backend for voice cloning. */
export function startBackendRecording(): Promise<void> {
  return invoke("start_backend_recording");
}

/** Stops backend recording and returns captured PCM f32 samples and sample rate. */
export function stopBackendRecording(): Promise<[number[], number]> {
  return invoke("stop_backend_recording");
}

/** Adds a custom cloned voice from an existing audio file path. */
export function addVoiceFromFile(
  name: string,
  filePath: string
): Promise<UnifiedVoiceDto> {
  return invoke("add_voice_from_file", { name, filePath });
}

/** Adds a custom cloned voice from recorded audio buffer. */
export function addVoiceFromRecording(
  name: string,
  pcmF32: number[],
  sampleRate: number
): Promise<UnifiedVoiceDto> {
  return invoke("add_voice_from_recording", {
    name,
    pcmF32,
    sampleRate,
  });
}

/** Deletes a custom cloned voice. */
export function deleteVoice(id: string): Promise<void> {
  return invoke("delete_voice", { id });
}

/** Renames a custom cloned voice. */
export function renameVoice(id: string, name: string): Promise<void> {
  return invoke("rename_voice", { id, name });
}

/** Converts a backend absolute preview path into a browser-playable asset URL. */
export function voicePreviewUrl(absPath: string): string {
  return convertFileSrc(absPath);
}

