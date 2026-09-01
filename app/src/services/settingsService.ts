import { invoke } from "@tauri-apps/api/core";
import type {
  VoxSettings,
  ModelCatalog,
  LlmProviderConfig,
  SttProviderConfig,
  TtsProviderConfig,
  ModelCapabilities,
  LlmModelInfo,
} from "@/store/settingsStore";

/** Mirror of `BootState` (ipc/settings.rs:17). */
export interface BootState {
  settings: VoxSettings;
  models_dir_exists: boolean;
  settings_path: string;
}

/** Mirror of `SettingUpdateResult` (ipc/settings.rs:33). */
export interface SettingUpdateResult {
  applied: boolean;
  reload_policy: string;
  message: string;
}

export interface AudioDevice {
  name: string;
  is_default: boolean;
}

/** Full settings snapshot, model paths, and directory health (ipc/settings/catalog.rs:28). */
export function getSettings(): Promise<BootState> {
  return invoke("get_settings");
}

/** Model catalog (ipc/settings/catalog.rs:55). */
export function requestModelCatalog(): Promise<ModelCatalog> {
  return invoke("get_model_catalog");
}

/**
 * Persist a single setting. Domain/key must match the backend convention
 * (e.g. "ui"/"theme", snake_case keys). Returns the reload policy.
 * (ipc/settings.rs:98)
 */
export function updateSetting(domain: string, key: string, value: unknown): Promise<SettingUpdateResult> {
  return invoke("update_setting", { domain, key, value });
}

/** Reset settings to defaults, returns the new settings (ipc/settings.rs:301). */
export function resetSettings(): Promise<VoxSettings> {
  return invoke("reset_settings");
}

/** Unified provider health check across LLM, STT, and TTS. */
export function checkProviderHealth(
  kind: "llm" | "stt" | "tts",
  provider?: LlmProviderConfig | SttProviderConfig | TtsProviderConfig
): Promise<boolean> {
  return invoke("check_provider_health", { kind, provider });
}

/** Health-check the LLM provider; falls back to saved config when omitted. */
export function checkLlmProviderHealth(provider?: LlmProviderConfig): Promise<boolean> {
  return checkProviderHealth("llm", provider);
}

/** Health-check the STT provider; falls back to saved config when omitted. */
export function checkSttProviderHealth(provider?: SttProviderConfig): Promise<boolean> {
  return checkProviderHealth("stt", provider);
}

/** Health-check the TTS provider; falls back to saved config when omitted. */
export function checkTtsProviderHealth(provider?: TtsProviderConfig): Promise<boolean> {
  return checkProviderHealth("tts", provider);
}

/** List models for a provider; falls back to saved config when omitted (ipc/settings.rs:801). */
export function listLlmModels(provider?: LlmProviderConfig): Promise<LlmModelInfo[]> {
  return invoke("list_llm_models", { provider });
}

export interface ModelProbeResult {
  capabilities: ModelCapabilities;
  validated_cap: number | null;
  cached_map: Record<string, ModelCapabilities>;
}

/** Probe capabilities for a remote model (returns ModelCapabilities). */
export async function probeModelCapabilities(
  provider?: LlmProviderConfig,
  modelId?: string,
  targetCap?: number
): Promise<ModelCapabilities> {
  const res = await invoke<ModelProbeResult>("probe_model_capabilities", {
    provider,
    modelId,
    targetCap,
  });
  return res.capabilities;
}

/** Probe capabilities for a remote model and return full result including updated cached map. */
export function probeModelCapabilitiesFull(
  provider?: LlmProviderConfig,
  modelId?: string,
  targetCap?: number
): Promise<ModelProbeResult> {
  return invoke<ModelProbeResult>("probe_model_capabilities", {
    provider,
    modelId,
    targetCap,
  });
}

/** Validate output token cap against model ceiling. */
export async function validateLlmTokenCap(
  provider: LlmProviderConfig | undefined,
  modelId: string | undefined,
  targetCap: number
): Promise<number | null> {
  const res = await invoke<ModelProbeResult>("probe_model_capabilities", {
    provider,
    modelId,
    targetCap,
  });
  return res.validated_cap;
}

/** List audio input or output devices (ipc/audio.rs). */
export function listAudioDevices(kind: "input" | "output" = "input"): Promise<AudioDevice[]> {
  return invoke("list_audio_devices", { kind });
}

export function listInputDevices(): Promise<AudioDevice[]> {
  return listAudioDevices("input");
}

/** Mark setup wizard as completed (ipc/settings.rs). */
export function completeSetupWizard(): Promise<void> {
  return invoke("complete_setup_wizard");
}
