import { invoke } from "./transport";
import type {
  VoxSettings,
} from "@/store/settingsStore";

export interface BootState {
  settings: VoxSettings;
  models_dir_exists: boolean;
  settings_path: string;
}

export interface SettingUpdateResult {
  applied: boolean;
  reload_policy: string;
  message: string;
  restart_scheduled: boolean;
}

export interface RestartStatusDto {
  restarting: boolean;
  target: string | null;
  timestamp: number;
}

export function getRestartStatus(): Promise<RestartStatusDto> {
  return invoke<RestartStatusDto>("get_restart_status");
}

export function getSettings(): Promise<BootState> {
  return invoke("get_settings");
}

export function updateSetting(
  domain: string,
  key: string,
  value: unknown
): Promise<SettingUpdateResult> {
  return invoke<SettingUpdateResult>("update_setting", { domain, key, value });
}

export function applySettingsPatch(patch: Record<string, unknown>): Promise<SettingUpdateResult> {
  return invoke<SettingUpdateResult>("apply_settings_patch", { patch });
}

export function updateModelParameters(
  modelId: string,
  patch: Record<string, unknown>
): Promise<SettingUpdateResult> {
  return updateSetting("model_parameters", modelId, patch);
}

export interface ResetSettingsResult {
  settings: VoxSettings;
  reload_policy: string;
  message: string;
  restart_scheduled: boolean;
}

export function resetSettings(): Promise<ResetSettingsResult> {
  return invoke<ResetSettingsResult>("reset_settings");
}



// Merged from former settingsContractService.ts (settings.rs -> settingsService.ts)
import type { ModelGroup } from "./setupService";

export type SettingReloadPolicy = "hot" | "worker_command" | "restart";

export interface SecretKeyContract {
  domain: string;
  key: string;
}

export interface LlmCloudProviderContract {
  id: string;
  name: string;
  base_url: string;
  key_placeholder?: string;
  popular?: boolean;
  tagline?: string;
  logo?: string;
  url?: string;
  keyPlaceholder?: string;
}

export interface NumericDefaultsContract {
  temperature: number;
  max_output_tokens: number;
  max_tokens: number;
  speed: number;
}

export interface SettingsContract {
  supported_keys: Record<string, string[]>;
  supported_keys_by_domain?: Record<string, string[]>;
  reload_policies: Record<string, Record<string, SettingReloadPolicy>>;
  secret_keys: SecretKeyContract[];
  llm_cloud_providers: LlmCloudProviderContract[];
  tts_providers: ModelGroup[];
  tts_models?: ModelGroup[];
  realtime_providers?: Array<{
    id: string;
    name: string;
    url: string;
    key_placeholder?: string;
    tagline?: string;
    logo?: string;
    voice_field: string;
    supports_web_search: boolean;
    supports_agent_mode: boolean;
  }>;
  defaults: NumericDefaultsContract;
  numeric_defaults?: NumericDefaultsContract;
}

let cachedContract: SettingsContract | null = null;
let contractFetchPromise: Promise<SettingsContract> | null = null;

export async function getSettingsContract(): Promise<SettingsContract> {
  if (cachedContract) return cachedContract;
  if (!contractFetchPromise) {
    contractFetchPromise = invoke<SettingsContract>("get_settings_contract")
      .then((contract) => {
        if (contract.llm_cloud_providers) {
          for (const p of contract.llm_cloud_providers) {
            p.url = p.base_url;
            p.keyPlaceholder = p.key_placeholder ?? "sk-...";
          }
        }
        cachedContract = contract;
        return contract;
      })
      .finally(() => {
        contractFetchPromise = null;
      });
  }
  return contractFetchPromise;
}

export function getCachedSettingsContract(): SettingsContract | null {
  return cachedContract;
}

export function isRestartKey(domain: string, key: string): boolean {
  return cachedContract?.reload_policies?.[domain]?.[key] === "restart";
}

export function getReloadPolicy(domain: string, key: string): SettingReloadPolicy {
  if (cachedContract?.reload_policies?.[domain]?.[key]) {
    return cachedContract.reload_policies[domain][key];
  }
  return isRestartKey(domain, key) ? "restart" : "hot";
}

export function getSupportedKeys(domain: string): readonly string[] {
  if (cachedContract?.supported_keys?.[domain]) {
    return cachedContract.supported_keys[domain];
  }
  return [];
}

export function isSecretKey(domain: string, key: string): boolean {
  return Boolean(cachedContract?.secret_keys?.some((s) => s.domain === domain && s.key === key));
}

export const settingsContractService = {
  getSettingsContract,
  getCachedSettingsContract,
  isRestartKey,
  getReloadPolicy,
  getSupportedKeys,
  isSecretKey,
};

