import { create } from "zustand";
import {
  listLlmModels,
  readModelCapabilitiesCache,
  requestModelCatalog,
} from "@/services/catalogService";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
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

export interface CatalogState {
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  remoteModelsFetchedKey: string | null;
  capabilitiesCache: Record<string, ModelCapabilities>;
  capabilitiesCacheError: string | null;
  probingMap: Record<string, CatalogProbingEntry>;
  modelCatalog: ModelCatalog | null;

  loadModelCatalog: () => Promise<void>;
  loadRemoteModels: (providerConfig?: LlmProviderConfig, force?: boolean) => Promise<void>;
  clearRemoteModels: () => void;
  clearRemoteModelsError: () => void;
  loadCapabilitiesCache: () => Promise<void>;
  patchCapabilities: (modelId: string, caps: ModelCapabilities) => void;
  setProbingEntry: (modelId: string, entry: CatalogProbingEntry) => void;
}

const remoteModelsInflight = new Map<string, Promise<LlmModelInfo[]>>();

export const useCatalogStore = create<CatalogState>((set, get) => ({
  remoteModels: [],
  loadingRemoteModels: false,
  remoteModelsError: null,
  remoteModelsFetchedKey: null,
  capabilitiesCache: {},
  capabilitiesCacheError: null,
  probingMap: {},
  modelCatalog: null,

  loadModelCatalog: async () => {
    try {
      const catalog = await requestModelCatalog();
      set({ modelCatalog: catalog });
      useSettingsStore.setState({ modelCatalog: catalog });
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