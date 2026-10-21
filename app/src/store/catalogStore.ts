import { create } from "zustand";
import {
  listLlmModels,
  readModelCapabilitiesCache,
  requestModelCatalog,
} from "@/services/catalogService";
import type {
  CloudProvider,
  TtsProviderEntry,
} from "@/data/providersCopy";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import {
  getEffectiveCloudApiKey,
  getEffectiveLlmServers,
} from "@/shared/lib/activeSelection";
import { useSettingsStore, selectActiveLlmProvider } from "@/store/settingsStore";
import type {
  LlmModelInfo,
  LlmProviderConfig,
  ModelCapabilities,
  ModelCatalog,
} from "@/store/settingsStore";

export interface CatalogProbingEntry {
  status: "idle" | "testing" | "success" | "error";
  capabilities?: ModelCapabilities;
  error?: string;
}

/**
 * Lifecycle of one provider's remote model list.
 *
 * `ready` means a fetch completed. `fetchedKey` holds the identity of the
 * cached result (kind|url|key presence); `null` means nothing is cached, so
 * the next load retries.
 */
interface ProviderRemoteModelsStatus {
  status: "idle" | "loading" | "ready" | "error";
  error: string | null;
  /** Identity of the cached result. `null` when nothing is cached. */
  fetchedKey: string | null;
}

/** Resolved endpoint of a provider, independent of the caller's config. */
interface ResolvedProviderTarget {
  kind: "cloud" | "server";
  base_url: string;
  api_key?: string;
  provider_name?: string;
}

export interface CatalogState {
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  remoteModelsFetchedKey: string | null;
  /**
   * Remote model list per provider id (cloud provider id or custom server id).
   * Independent of the active-provider flow above: every connected provider
   * gets its own group, fetched lazily and cached per provider.
   */
  remoteModelsByProvider: Record<string, LlmModelInfo[]>;
  /** Per-provider lifecycle of the list above (loading / ready / error). */
  remoteModelsStatusByProvider: Record<string, ProviderRemoteModelsStatus>;
  capabilitiesCache: Record<string, ModelCapabilities>;
  capabilitiesCacheError: string | null;
  probingMap: Record<string, CatalogProbingEntry>;
  modelCatalog: ModelCatalog | null;
  /** Store-owned provider lists (SSOT). Populated from the settings contract. */
  cloudProviders: CloudProvider[];
  ttsProviders: TtsProviderEntry[];
  setProviders: (cloud: CloudProvider[], tts: TtsProviderEntry[]) => void;

  loadModelCatalog: () => Promise<void>;
  loadRemoteModels: (providerConfig?: LlmProviderConfig, force?: boolean) => Promise<void>;
  clearRemoteModels: () => void;
  clearRemoteModelsError: () => void;
  /**
   * Loads (and caches) the remote model list of one provider. The provider
   * config is optional: without it the endpoint and the API key are resolved
   * from the settings inventory and the backend contract.
   *
   * Never fires a request for a cloud provider without a stored key.
   * Concurrent calls for the same provider share one in-flight request.
   */
  loadRemoteModelsForProvider: (
    providerId: string,
    providerConfig?: LlmProviderConfig,
    force?: boolean
  ) => Promise<void>;
  /** Drops the cached list of one provider so the next load refetches it. */
  invalidateRemoteModelsForProvider: (providerId: string) => void;
  loadCapabilitiesCache: () => Promise<void>;
  patchCapabilities: (modelId: string, caps: ModelCapabilities) => void;
  setProbingEntry: (modelId: string, entry: CatalogProbingEntry) => void;
}

const remoteModelsInflight = new Map<string, Promise<LlmModelInfo[]>>();
/** In-flight per-provider requests, keyed by provider id. */
const providerRemoteModelsInflight = new Map<string, Promise<LlmModelInfo[]>>();

/**
 * Resolves the endpoint of one provider.
 *
 * The caller's config wins for the endpoint, but a missing endpoint or key is
 * filled from the settings inventory (custom servers) and from the backend
 * contract (cloud providers). The stored key wins over the passed one: the
 * draft is the newer truth than a stale config object.
 */
function resolveProviderTarget(
  providerId: string,
  providerConfig: LlmProviderConfig | undefined,
  cloudProviders: CloudProvider[]
): ResolvedProviderTarget | null {
  if (providerConfig?.kind === "embedded") return null;

  const settingsState = useSettingsStore.getState();
  const draft = settingsState.draftSettings ?? null;
  const committed = settingsState.settings ?? null;

  const contractEntry = settingsState.contract?.llm_cloud_providers.find(
    (p) => p.id === providerId
  );
  const storeEntry = cloudProviders.find((p) => p.id === providerId);
  const contractBaseUrl =
    contractEntry?.base_url || contractEntry?.url || storeEntry?.url || "";

  const storedKey = getEffectiveCloudApiKey(draft, committed, providerId)?.trim() || undefined;
  const passedKey = providerConfig?.api_key?.trim() || undefined;
  const apiKey = storedKey || passedKey;

  if (providerConfig) {
    return {
      kind: providerConfig.kind === "server" ? "server" : "cloud",
      base_url: providerConfig.base_url?.trim() || contractBaseUrl,
      api_key: apiKey,
      provider_name: providerConfig.provider_name,
    };
  }

  const server = getEffectiveLlmServers(draft, committed).find((s) => s.id === providerId);
  if (server) {
    return {
      kind: "server",
      base_url: (server.url || "").trim(),
      api_key: server.api_key?.trim() || apiKey,
      provider_name: server.name,
    };
  }

  if (contractBaseUrl) {
    return {
      kind: "cloud",
      base_url: contractBaseUrl,
      api_key: apiKey,
      provider_name: contractEntry?.name || storeEntry?.name,
    };
  }

  return null;
}

export const useCatalogStore = create<CatalogState>((set, get) => ({
  remoteModels: [],
  loadingRemoteModels: false,
  remoteModelsError: null,
  remoteModelsFetchedKey: null,
  remoteModelsByProvider: {},
  remoteModelsStatusByProvider: {},
  capabilitiesCache: {},
  capabilitiesCacheError: null,
  probingMap: {},
  modelCatalog: null,
  cloudProviders: [],
  ttsProviders: [],

  setProviders: (cloud, tts) => {
    set({ cloudProviders: cloud, ttsProviders: tts });
  },

  loadModelCatalog: async () => {
    try {
      const catalog = await requestModelCatalog();
      set({ modelCatalog: catalog });
    } catch (err) {
      console.error("Failed to load model catalog:", err);
    }
  },

  loadRemoteModels: async (providerConfig?: LlmProviderConfig, force = false) => {
    const provider = providerConfig || selectActiveLlmProvider(useSettingsStore.getState());
    if (!provider || provider.kind === "embedded" || !provider.base_url) {
      if (get().remoteModels.length > 0 || get().remoteModelsError) {
        set({ remoteModels: [], remoteModelsError: null, loadingRemoteModels: false });
      }
      return;
    }

    if (provider.kind === "cloud" && !provider.api_key?.trim()) {
      set({
        remoteModels: [],
        remoteModelsError: "API key required. Configure your key in Providers.",
        loadingRemoteModels: false,
      });
      return;
    }

    const fetchKey = `${provider.kind}|${provider.provider_id ?? ""}|${provider.base_url}|${provider.api_key ? "key" : "nokey"}`;
    if (!force && get().remoteModelsFetchedKey === fetchKey && get().remoteModels.length > 0) {
      return;
    }

    const inflight = remoteModelsInflight.get(fetchKey);
    if (!force && inflight) {
      try {
        await inflight;
      } catch {
        // Owner records error
      }
      return;
    }

    console.info(
      `[Providers] loadRemoteModels kind=${provider.kind} url=${provider.base_url} hasKey=${Boolean(provider.api_key)} force=${force}`
    );
    // Clear stale models immediately when switching providers to prevent showing previous provider's models
    set({
      remoteModels: [],
      loadingRemoteModels: true,
      remoteModelsError: null,
    });
    const request = listLlmModels(provider);
    remoteModelsInflight.set(fetchKey, request);
    try {
      const list = await request;
      console.info(
        `[Providers] loadRemoteModels ok kind=${provider.kind} url=${provider.base_url} count=${list.length}`
      );
      set({
        remoteModels: list,
        remoteModelsFetchedKey: fetchKey,
        loadingRemoteModels: false,
        remoteModelsError: null,
      });
    } catch (err) {
      const msg = ipcErrorMessage(err);
      console.error("[Providers] loadRemoteModels failed:", err);
      set({
        remoteModels: [],
        loadingRemoteModels: false,
        remoteModelsError: msg,
      });
    } finally {
      if (remoteModelsInflight.get(fetchKey) === request) {
        remoteModelsInflight.delete(fetchKey);
      }
    }
  },

  clearRemoteModels: () => {
    set({
      remoteModels: [],
      loadingRemoteModels: false,
      remoteModelsError: null,
    });
  },

  clearRemoteModelsError: () => {
    if (get().remoteModelsError) {
      set({ remoteModelsError: null });
    }
  },

  loadRemoteModelsForProvider: async (
    providerId: string,
    providerConfig?: LlmProviderConfig,
    force = false
  ) => {
    const id = providerId.trim() ?? "";
    if (!id) return;

    const target = resolveProviderTarget(id, providerConfig, get().cloudProviders);

    // A degraded group: empty list plus an error the UI can offer a retry for.
    const degrade = (error: string) => {
      const current = get().remoteModelsStatusByProvider[id];
      const currentModels = get().remoteModelsByProvider[id];
      // Already degraded the same way: no new object identities, no re-render.
      if (
        current.status === "error" &&
        current.error === error &&
        currentModels.length === 0
      ) {
        return;
      }
      console.info(`[Catalog] loadRemoteModelsForProvider degraded id=${id}: ${error}`);
      set((state) => ({
        remoteModelsByProvider: { ...state.remoteModelsByProvider, [id]: [] },
        remoteModelsStatusByProvider: {
          ...state.remoteModelsStatusByProvider,
          [id]: { status: "error", error, fetchedKey: null },
        },
      }));
    };

    if (!target) {
      degrade("Provider endpoint is not configured.");
      return;
    }
    // RULE: a provider without a stored key never reaches the network.
    if (target.kind === "cloud" && !target.api_key) {
      degrade("API key required. Configure your key in Providers.");
      return;
    }
    if (!target.base_url) {
      degrade("Provider endpoint is not configured.");
      return;
    }

    const fetchKey = `${target.kind}|${id}|${target.base_url}|${target.api_key ? "key" : "nokey"}`;
    const cached = get().remoteModelsStatusByProvider[id];
    if (!force && cached.status === "ready" && cached.fetchedKey === fetchKey) {
      return;
    }

    // One in-flight request per provider: concurrent callers join it.
    const inflight = providerRemoteModelsInflight.get(id);
    if (!force && inflight) {
      try {
        await inflight;
      } catch {
        // The owner records the error.
      }
      return;
    }

    console.info(
      `[Catalog] loadRemoteModelsForProvider id=${id} kind=${target.kind} url=${target.base_url} hasKey=${Boolean(target.api_key)} force=${force}`
    );
    set((state) => ({
      remoteModelsStatusByProvider: {
        ...state.remoteModelsStatusByProvider,
        [id]: { status: "loading", error: null, fetchedKey: cached.fetchedKey ?? null },
      },
    }));

    const request = listLlmModels({
      kind: target.kind,
      provider_id: id,
      base_url: target.base_url,
      model: providerConfig?.model ?? "",
      api_key: target.api_key,
      provider_name: target.provider_name,
    });
    providerRemoteModelsInflight.set(id, request);
    try {
      const list = await request;
      console.info(`[Catalog] loadRemoteModelsForProvider ok id=${id} count=${list.length}`);
      set((state) => ({
        remoteModelsByProvider: { ...state.remoteModelsByProvider, [id]: list },
        remoteModelsStatusByProvider: {
          ...state.remoteModelsStatusByProvider,
          [id]: { status: "ready", error: null, fetchedKey: fetchKey },
        },
      }));
    } catch (err) {
      const msg = ipcErrorMessage(err);
      console.error(`[Catalog] loadRemoteModelsForProvider failed for ${id}:`, err);
      set((state) => ({
        remoteModelsByProvider: { ...state.remoteModelsByProvider, [id]: [] },
        remoteModelsStatusByProvider: {
          ...state.remoteModelsStatusByProvider,
          [id]: { status: "error", error: msg, fetchedKey: null },
        },
      }));
    } finally {
      if (providerRemoteModelsInflight.get(id) === request) {
        providerRemoteModelsInflight.delete(id);
      }
    }
  },

  invalidateRemoteModelsForProvider: (providerId: string) => {
    const id = providerId.trim() ?? "";
    if (!id) return;
    set((state) => {
      const models = { ...state.remoteModelsByProvider };
      delete models[id];
      const statuses = { ...state.remoteModelsStatusByProvider };
      delete statuses[id];
      return { remoteModelsByProvider: models, remoteModelsStatusByProvider: statuses };
    });
  },

  loadCapabilitiesCache: async () => {
    try {
      const res = await readModelCapabilitiesCache();
      set({
        capabilitiesCache: res.cached_map || {},
        capabilitiesCacheError: res.cache_error || null,
      });
    } catch (err) {
      const msg = ipcErrorMessage(err);
      console.error("Failed to load capabilities cache:", err);
      set({ capabilitiesCacheError: msg });
    }
  },

  patchCapabilities: (modelId: string, caps: ModelCapabilities) => {
    set((state) => ({
      remoteModels: state.remoteModels.map((m) =>
        m.id === modelId ? { ...m, capabilities: caps } : m
      ),
      capabilitiesCache: {
        ...state.capabilitiesCache,
        [`${caps.provider_kind}:${caps.model_id}`]: caps,
      },
    }));
  },

  setProbingEntry: (modelId: string, entry: CatalogProbingEntry) => {
    set((state) => ({
      probingMap: { ...state.probingMap, [modelId]: entry },
    }));
  },
}));