import { invoke } from "@tauri-apps/api/core";
import type {
  VoxSettings,
  ModelCatalog,
  LlmProviderConfig,
  SttProviderConfig,
  ModelCapabilities,
  LlmModelInfo,
  ProviderCaps,
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

/** Settings capabilities for a TTS provider id (ipc/settings/catalog.rs).
 * Returns null when the backend is unreachable or rejects the id. A fabricated
 * fallback would render controls for capabilities that may not exist. */
export async function getProviderCaps(providerId: string): Promise<ProviderCaps | null> {
  try {
    return await invoke<ProviderCaps>("get_provider_caps", { provider_id: providerId });
  } catch (err) {
    console.error(`[Settings] get_provider_caps failed for ${providerId}:`, err);
    return null;
  }
}

/**
 * Persist a single setting. Domain/key must match the backend convention
 * (e.g. "ui"/"theme", snake_case keys). Returns the reload policy.
 * (ipc/settings.rs:98)
 */
export function updateSetting(
  domain: string,
  key: string,
  value: unknown
): Promise<SettingUpdateResult> {
  return invoke<SettingUpdateResult>("update_setting", { domain, key, value });
}

/** Reset all settings to factory defaults. (ipc/settings/mutation.rs:267) */
export function resetSettings(): Promise<VoxSettings> {
  return invoke<VoxSettings>("reset_settings");
}

export interface ProviderHealthCheckResult {
  healthy: boolean;
  dialect?: string | null;
}

/** Check health/connectivity for a specific provider. */
export async function checkLlmProviderHealth(
  provider?: LlmProviderConfig
): Promise<ProviderHealthCheckResult> {
  return invoke<ProviderHealthCheckResult>("check_provider_health", { kind: "llm", provider });
}

export async function checkSttProviderHealth(
  provider?: SttProviderConfig
): Promise<ProviderHealthCheckResult> {
  return invoke<ProviderHealthCheckResult>("check_provider_health", { kind: "stt", provider });
}

/** Check health/connectivity for the saved active TTS provider. No payload:
 * the backend derives the config from its own settings via to_provider_config. */
export async function checkTtsProviderHealth(): Promise<ProviderHealthCheckResult> {
  return invoke<ProviderHealthCheckResult>("check_provider_health", { kind: "tts" });
}

/** Fetch dynamic list of models available from a remote or local LLM server. */
export async function listLlmModels(provider?: LlmProviderConfig): Promise<LlmModelInfo[]> {
  return invoke<LlmModelInfo[]>("list_llm_models", { provider });
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
    model_id: modelId,
    target_cap: targetCap,
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
    model_id: modelId,
    target_cap: targetCap,
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
    model_id: modelId,
    target_cap: targetCap,
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

export interface RemoteServerConfig {
  connectionString: string;
  sshPort: number | null;
  identityKeyPath: string | null;
  remotePath: string;
  serverPort: number;
}

export function setupRemoteServer(config: RemoteServerConfig): Promise<void> {
  return invoke("setup_remote_server", {
    connection_string: config.connectionString,
    ssh_port: config.sshPort,
    identity_key_path: config.identityKeyPath,
    remote_path: config.remotePath,
    server_port: config.serverPort,
  });
}

