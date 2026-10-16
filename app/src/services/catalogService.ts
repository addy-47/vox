import { invoke } from "./transport";
import type {
  ModelCatalog,
  ProviderCaps,
  VoxSettings,
  LlmModelInfo,
  LlmProviderConfig,
  ModelCapabilities,
  ProviderHealthCheckResult,
} from "@/store/settingsStore";

// ---- Provider health & model catalog (ipc/catalog.rs) ----

export interface ModelProbeResult {
  capabilities: ModelCapabilities;
  validated_cap: number | null;
  cached_map: Record<string, ModelCapabilities>;
  cache_error: string | null;
}

export interface CapabilityCacheRead {
  cached_map: Record<string, ModelCapabilities>;
  cache_error: string | null;
}

export async function checkLlmProviderHealth(
  provider?: LlmProviderConfig
): Promise<ProviderHealthCheckResult> {
  return invoke<ProviderHealthCheckResult>("check_provider_health", { kind: "llm", provider });
}

export async function checkTtsProviderHealth(): Promise<ProviderHealthCheckResult> {
  return invoke<ProviderHealthCheckResult>("check_provider_health", { kind: "tts" });
}

export function requestModelCatalog(): Promise<ModelCatalog> {
  return invoke("get_model_catalog");
}

export async function getProviderCaps(providerId: string): Promise<ProviderCaps | null> {
  try {
    return await invoke<ProviderCaps>("get_provider_caps", { providerId });
  } catch (err: unknown) {
    console.error(`[Catalog] get_provider_caps failed for ${providerId}:`, err);
    return null;
  }
}

export async function listLlmModels(provider?: LlmProviderConfig): Promise<LlmModelInfo[]> {
  return invoke<LlmModelInfo[]>("list_llm_models", { provider });
}

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

export function readModelCapabilitiesCache(): Promise<CapabilityCacheRead> {
  return invoke<CapabilityCacheRead>("get_model_capabilities_cache");
}

export interface ProbeProviderTargetParams {
  kind: string;
  providerId?: string | null;
  baseUrl?: string | null;
  apiKey?: string | null;
  model?: string | null;
}

export interface ProbeProviderTargetResult {
  healthy: boolean;
  dialect?: string | null;
  reason?: string | null;
  models: LlmModelInfo[];
}

export async function probeProviderTarget(
  params: ProbeProviderTargetParams
): Promise<ProbeProviderTargetResult> {
  try {
    return await invoke<ProbeProviderTargetResult>("probe_provider_target", {
      kind: params.kind,
      providerId: params.providerId ?? null,
      baseUrl: params.baseUrl ?? null,
      apiKey: params.apiKey ?? null,
      model: params.model ?? null,
    });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return {
      healthy: false,
      dialect: null,
      reason: message || "Connection probe failed",
      models: [],
    };
  }
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

export function listFavoriteModels(): Promise<string[]> {
  return invoke("list_favorite_models");
}

export function toggleModelFavorite(modelId: string): Promise<boolean> {
  return invoke("toggle_model_favorite", { modelId });
}

export interface ProviderHealthCheckResult {
  healthy: boolean;
  dialect?: string | null;
  reason?: string | null;
}

// ---- settings-contract helpers also live in catalog contract responses? ----
// Kept in settingsService.ts; catalogService does not own the settings contract.