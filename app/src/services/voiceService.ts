import { invoke } from "@tauri-apps/api/core";

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

export function listVoices(provider?: "custom" | "edge" | "kokoro" | "supertonic"): Promise<VoiceEntryDto[]> {
  return invoke("list_voices", { provider });
}

export function renameVoice(id: string, name: string): Promise<void> {
  return invoke("rename_voice", { id, name });
}

export function addVoiceFromFile(name: string, filePath: string): Promise<VoiceEntryDto> {
  return invoke("add_voice_from_file", { name, file_path: filePath });
}

export function addVoiceFromRecording(name: string, pcmF32: number[], sampleRate: number): Promise<VoiceEntryDto> {
  return invoke("add_voice_from_recording", { name, pcm_f32: pcmF32, sample_rate: sampleRate });
}

export function deleteVoice(id: string): Promise<void> {
  return invoke("delete_voice", { id });
}
