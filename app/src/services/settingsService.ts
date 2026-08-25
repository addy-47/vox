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

/** Full settings snapshot + dir health, called on app mount (ipc/settings.rs:47). */
export function requestBootState(): Promise<BootState> {
  return invoke("request_boot_state");
}

/** Model catalog (ipc/settings.rs:67). */
export function requestModelCatalog(): Promise<ModelCatalog> {
  return invoke("request_model_catalog");
}

/** Current settings (ipc/settings.rs:79). */
export function getSettings(): Promise<VoxSettings> {
  return invoke("get_settings");
}

/**
 * Persist a single setting. Domain/key must match the backend convention
 * (e.g. "ui"/"theme", snake_case keys). Returns the reload policy.
 * (ipc/settings.rs:98)
 */
export function updateSetting(domain: string, key: string, value: unknown): Promise<SettingUpdateResult> {
  return invoke("update_setting", { domain, key, value });
}

/** Update main/tray interaction mode (Passive/PTT) (ipc/tray.rs:175). */
export function updateInteractionMode(target: "main" | "tray", mode: "Passive" | "PTT"): Promise<void> {
  return invoke("update_interaction_mode", { target, mode });
}

/** Reset settings to defaults, returns the new settings (ipc/settings.rs:301). */
export function resetSettings(): Promise<VoxSettings> {
  return invoke("reset_settings");
}

/** Health-check the LLM provider; falls back to saved config when omitted (ipc/settings.rs:643). */
export function checkLlmProviderHealth(provider?: LlmProviderConfig): Promise<boolean> {
  return invoke("check_llm_provider_health", { provider });
}

/** Health-check the STT provider; falls back to saved config when omitted (ipc/settings.rs:707). */
export function checkSttProviderHealth(provider?: SttProviderConfig): Promise<boolean> {
  return invoke("check_stt_provider_health", { provider });
}

/** Health-check the TTS provider; falls back to saved config when omitted (ipc/settings.rs:750). */
export function checkTtsProviderHealth(provider?: TtsProviderConfig): Promise<boolean> {
  return invoke("check_tts_provider_health", { provider });
}

/** List models for a provider; falls back to saved config when omitted (ipc/settings.rs:801). */
export function listLlmModels(provider?: LlmProviderConfig): Promise<LlmModelInfo[]> {
  return invoke("list_llm_models", { provider });
}

/** Get cached model capabilities from disk cache (~/.vox/cache/model_capabilities.json). */
export function getCachedCapabilities(): Promise<Record<string, ModelCapabilities>> {
  return invoke("get_cached_capabilities");
}

/** Probe capabilities for a remote model (ipc/settings.rs:849). */
export function probeModelCapabilities(
  provider?: LlmProviderConfig,
  modelId?: string
): Promise<ModelCapabilities> {
  return invoke("probe_model_capabilities", { provider, modelId });
}

/** List audio input devices (ipc/audio.rs). */
export function listInputDevices(): Promise<AudioDevice[]> {
  return invoke("list_input_devices");
}

/** List audio output devices (ipc/audio.rs). */
export function listOutputDevices(): Promise<AudioDevice[]> {
  return invoke("list_output_devices");
}

/** Mark setup wizard as completed (ipc/settings.rs). */
export function completeSetupWizard(): Promise<void> {
  return invoke("complete_setup_wizard");
}
