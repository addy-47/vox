import { create } from "zustand";
import {
  getSettings,
  getRestartStatus,
  updateSetting,
  resetSettings,
} from "@/services/settingsService";
import {
  readModelCapabilitiesCache,
  listLlmModels,
  requestModelCatalog,
} from "@/services/catalogService";
import { applyTheme } from "@/shared/theme";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import { DOMAIN_DIRTY_KEYS, SETTINGS_SCOPE_KEYS, type SettingsDomainId, type SettingsScope } from "@/data/settingsCopy";
import {
  settingsContractService,
  type SettingsContract,
} from "@/services/settingsService";
import {
  updateCloudProvidersFromContract,
  updateTtsProvidersFromContract,
} from "@/data/providersCopy";

/** Reads a settings scope as a key-value map for dynamic key access. The
 * single guarded cast in the store; every dynamic read funnels through here. */
function scopeEntries(scope: unknown): Record<string, unknown> {
  if (typeof scope !== "object" || scope === null) return {};
  return scope as Record<string, unknown>;
}

export type PipelineMode = "modular" | "realtime";

/** Primary runtime tier shared by every model domain. */
export type ActiveCategory = "embedded" | "server" | "cloud";

/**
 * Mirrors the backend `ActiveSelection` (`app/src-tauri/src/config/types.rs`).
 * `provider` is null for plain embedded models, otherwise a provider name or a
 * server id from `providers.*.servers`.
 */
export interface ActiveSelection {
  category: ActiveCategory;
  provider: string | null;
  model: string;
}

/** Mirrors the backend `LastSelectedModels` (`config/types.rs`). */
export interface LastSelectedModels {
  embedded?: string;
  server?: string;
  cloud?: string;
}

/** Realtime configuration subkey (the `realtime.<subkey>` settings object). */
export type RealtimeSubkey = string;

export type LlmProviderKind = ActiveCategory;

export interface LlmProviderConfig {
  kind: LlmProviderKind;
  /** Inventory id (`providers.llm.servers[].id`) or cloud provider id. Never a display name. */
  provider_id?: string;
  base_url?: string;
  model?: string;
  api_key?: string;
  provider_name?: string;
}

export type SttProviderKind = "embedded";

export interface SttProviderConfig {
  kind: SttProviderKind;
  model_type?: string;
}

export type CapabilitySupport = "supported" | "unsupported" | "unknown";

export type CapabilityProvenance =
  | "catalog_baseline"
  | "family_baseline"
  | "probed_server"
  | "declared_static"
  | "user_configured"
  | "unknown";

export type ProbeOutcome = "measured" | "unsupported" | "failed" | "skipped";

export interface ProbeCheck {
  id: string;
  label: string;
  outcome: ProbeOutcome;
  detail?: string | null;
  duration_ms?: number | null;
}

export interface ModelCapabilities {
  model_id: string;
  provider_kind: string;
  supports_tools: CapabilitySupport;
  supports_latin: CapabilitySupport;
  supports_devanagari: CapabilitySupport;
  context_window?: number | null;
  max_output_tokens?: number | null;
  provenance: CapabilityProvenance;
  tps?: number | null;
  ttft_ms?: number | null;
  server_has_gpu: boolean;
  is_gpu_accelerated: boolean;
  gpu_status: string;
  vram_bytes?: number | null;
  parameter_size?: string | null;
  quantization?: string | null;
  family?: string | null;
  tested_at_epoch: number;
  checks: ProbeCheck[];
}

export interface LlmModelInfo {
  id: string;
  name: string;
  size_bytes: number | null;
  quantization: string | null;
  family: string | null;
  provider_kind: string;
  capabilities?: ModelCapabilities | null;
}

export interface ModelEntry {
  id: string;
  path: string;
  size: number;
  sha256: string;
  archive?: string | null;
  required?: boolean;
}

export interface ModelGroupInfo {
  id: string;
  name: string;
  category: string;
  subcategory?: string;
  description?: string;
  parameters?: string;
  ram_usage?: string;
  tradeoffs?: string;
  version: string;
  is_built_in?: boolean;
  is_cloud?: boolean;
  is_remote?: boolean;
  required?: boolean;
  files?: ModelEntry[];
}

export type TtsVoiceSource = "catalog" | "custom" | "edge" | "none";

export interface ParamRange {
  min: number;
  max: number;
  step: number;
}

export interface ProviderCaps {
  voices: TtsVoiceSource;
  clone: boolean;
  speed_range: ParamRange;
}

// Alias for backwards compatibility during component migration
export type ModelMetadata = ModelGroupInfo;

export interface VoiceProfile {
  id: number;
  name: string;
}

export interface ModelCatalog {
  llm: ModelGroupInfo[];
  stt: ModelGroupInfo[];
  tts: ModelGroupInfo[];
  vad: ModelGroupInfo[];
  auxiliary: ModelGroupInfo[];
  model_groups: ModelGroupInfo[];
  voices: VoiceProfile[];
  preset_colors: string[];
  active_voice_name?: string | null;
  /** Per-model parameter overrides from the `models` table (`ipc/catalog.rs`). */
  model_parameters: Record<string, Record<string, unknown>>;
}

export type AudioOutputMode = "Speaker" | "Headset";


export interface VadSettings {
  threshold: number;
  ptt_noise_gate: number;
  vad_backend: string;
  silence_duration_ms: number;
  speech_onset_ms: number;
  max_speech_duration_s: number;
}

export interface SttSettings {
  active: ActiveSelection;
  last_selected: LastSelectedModels;
  transliterate_enabled: boolean;
}

export interface LlmSettings {
  active: ActiveSelection;
  last_selected: LastSelectedModels;
  cloud_keys: Record<string, string>;
  /**
   * Draft-only vehicle for the custom server inventory. The backend persists
   * it to `providers.jsonc` (`update_setting("llm","servers",…)`); it is never
   * part of `settings.jsonc`, so it stays undefined until the user edits it.
   */
  servers?: LlmCustomServer[];
}

export interface TtsSettings {
  active: ActiveSelection;
  last_selected: LastSelectedModels;
  active_voice_id: string;
  /**
   * Draft-only vehicle for the TTS server inventory, persisted to
   * `providers.jsonc` like `LlmSettings.servers`.
   */
  servers?: TtsCustomServer[];
}

export interface RealtimeSettings {
  active: ActiveSelection;
  active_voice_id: string;
  /**
   * Draft-only vehicle for realtime provider API keys, persisted to
   * `providers.jsonc` (`update_setting("realtime","api_keys",…)`).
   */
  api_keys?: Record<string, string>;
}

export interface InteractionSettings {
  mode: "passive" | "ptt" | "Passive" | "PTT";
  pipeline_mode: PipelineMode;
}

export interface DictationSettings {
  enabled: boolean;
  interaction_mode: "passive" | "ptt";
  hotkey: string;
  output_mode: "paste" | "clipboard" | "tray";
  silence_auto_stop_ms?: number;
}

export interface WorkingMemorySettings {
  private_mode: boolean;
  auto_compaction: boolean;
  max_context_share: number;
  web_search_enabled: boolean;
}

export interface AppearanceSettings {
  theme: string;
  accent_seed: string;
}

export interface PersonalMemorySettings {
  context_retrieval_enabled: boolean;
  pipeline_processing_enabled: boolean;
  top_k_facts: number;
  semantic_similarity_cutoff: number;
  consolidation_cadence: string;
  consolidation_time: string;
  suggestion_policy?: "manual_review" | "auto_apply" | string;
}

export interface PersonaSettings {
  modular_prompt: string;
  realtime_prompt: string;
}

export interface SystemSettings {
  setup_completed: boolean;
}

export interface LlmCustomServer {
  id: string;
  name: string;
  url: string;
  model: string;
  protocol: string;
  api_key?: string | null;
}

export interface TtsCustomServer {
  id: string;
  name: string;
  url: string;
  remote_path: string;
  api_key?: string | null;
}

export interface ProvidersConfigFile {
  llm: {
    servers: LlmCustomServer[];
    cloud_keys: Record<string, string>;
  };
  tts: {
    servers: TtsCustomServer[];
  };
  realtime: {
    api_keys: Record<string, string>;
  };
}

export interface VoxSettings {
  vad: VadSettings;
  stt: SttSettings;
  llm: LlmSettings;
  tts: TtsSettings;
  realtime: RealtimeSettings;
  interaction: InteractionSettings;
  dictation: DictationSettings;
  working_memory: WorkingMemorySettings;
  appearance: AppearanceSettings;
  personal_memory: PersonalMemorySettings;
  persona: PersonaSettings;
  system: SystemSettings;
  providers?: ProvidersConfigFile;
}

/** Per-domain commit state, derived once per store mutation. */
export interface DomainFlags {
  dirty: boolean;
  requiresRestart: boolean;
}

/**
 * Every `(domain, key)` pair the backend `apply_setting_mutation` accepts
 * (`app/src-tauri/src/config/mutation.rs`). `updateDraft` only compiles for
 * these keys, so a phantom write like `("llm","server")` is a build error,
 * not a runtime rejection. `providers` maps to `never`: inventory writes go
 * through their domain vehicle (`llm.servers`, `llm.cloud_keys`, …).
 */
export interface DraftKeyMap {
  appearance: "theme" | "accent_seed";
  vad: "threshold" | "ptt_noise_gate" | "silence_duration_ms" | "speech_onset_ms" | "vad_backend";
  stt: "active" | "last_selected" | "transliterate_enabled";
  llm: "active" | "last_selected" | "servers" | "cloud_keys";
  tts: "active" | "last_selected" | "active_voice_id" | "voice" | "voice_id" | "servers";
  realtime: "active" | "active_voice_id" | "api_keys";
  interaction: "mode" | "pipeline_mode";
  dictation: "enabled" | "interaction_mode" | "hotkey" | "output_mode" | "silence_auto_stop_ms";
  working_memory: "private_mode" | "auto_compaction" | "max_context_share" | "web_search_enabled";
  personal_memory: "context_retrieval_enabled" | "pipeline_processing_enabled" | "top_k_facts" | "semantic_similarity_cutoff" | "consolidation_cadence" | "consolidation_time" | "suggestion_policy";
  persona: "modular_prompt" | "realtime_prompt";
  system: "setup_completed";
  providers: never;
}

export interface SettingsState {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  modelCatalog: ModelCatalog | null;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  remoteModelsFetchedKey: string | null;
  capabilitiesCache: Record<string, ModelCapabilities>;
  capabilitiesCacheError: string | null;
  isLoading: boolean;
  hasChanges: boolean;
  /**
   * `domain.key` entries the backend classified `SettingReloadPolicy::Restart`
   * during the most recent commit. Authoritative: the backend executed the
   * rebuild. The UI must never reconstruct this from a local key list.
   */
  restartKeys: string[];
  /** True while the backend's coalesced engine restart is running. */
  restartInFlight: boolean;
  error: string | null;
  /** Canonical settings contract from backend (supported keys, reload policies, providers, defaults). */
  contract: SettingsContract | null;

  loadContract: () => Promise<SettingsContract | null>;
  loadSettings: () => Promise<void>;
  loadModelCatalog: () => Promise<void>;
  loadRemoteModels: (providerConfig?: LlmProviderConfig, force?: boolean) => Promise<void>;
  clearRemoteModelsError: () => void;
  loadCapabilitiesCache: () => Promise<void>;
  patchRemoteModelCapabilities: (modelId: string, caps: ModelCapabilities) => void;
  updateDraft: <D extends keyof VoxSettings>(
    domain: D,
    key: DraftKeyMap[D],
    value: unknown,
    explicitDomainId?: SettingsDomainId
  ) => void;
  /**
   * Persists draft changes. Without `only`, commits every dirty key.
   * With `only`, commits exactly one key (e.g. an inventory delete from a
   * trash icon) so unrelated half-typed edits are never swept along.
   */
  commitChanges: (only?: { domain: keyof VoxSettings; key: string }) => Promise<void>;
  /**
   * Commit any pending autosave immediately (same guards as the 600ms timer).
   * Called when a settings card closes so hot changes survive instead of
   * dying with the timer. Restart-classified dirty state is left untouched
   * in the draft — survive, not apply, not prompt.
   */
  flushPendingAutosave: () => void;
  trackRestartCompletion: () => void;
  discardChanges: () => void;
  isDomainDirty: (domainId: string) => boolean;
  isDomainRequiringRestart: (domainId: string) => boolean;
  discardDomainChanges: (domainId: string) => void;
  isCategoryDirty: (category: string) => boolean;
  discardCategoryChanges: (category: string) => void;
  restoreDefaults: () => Promise<void>;
  toggleTheme: () => void;
  isCommitting: boolean;
  autoSavedDomain: string | null;
  lastSavedTimestamp: number;
  triggerAutoSaveToast: (domainId: string) => void;
  /**
   * Domains whose most recent commit was rejected by the backend.
   * Value is the rejected key list for that domain — it was a single flat
   * `failedSaveKeys` array shared by every domain, so two simultaneous
   * failures made one card's banner print the other card's key names.
   */
  failedSaveDomains: Record<string, string[]>;
  triggerSaveFailure: (domainId: string, keys: string[]) => void;
  /**
   * Transient desk-level error (provider health / model-list failure) shown
   * in the card footer like every other banner — never inline inside the
   * desk, which clutters the card. Owned by the desk that sets it; the card
   * wrapper only renders the entry matching its own domain.
   */
  deskFooter: { domain: string; title: string; detail: string } | null;
  setDeskFooter: (entry: { domain: string; title: string; detail: string } | null) => void;
  /**
   * Per-domain dirty / restart-required flags, recomputed ONCE per store
   * mutation instead of once per subscriber.
   *
   * `isDomainDirty` and `isDomainRequiringRestart` are `JSON.stringify` walks
   * over ~57 declared keys plus a whole-scope TTS comparison. They were invoked
   * directly as Zustand selectors, so every one of the six always-mounted
   * `SettingsCardWrapper`s ran both on every `set()` — roughly 1,100 stringify
   * calls per keystroke, inside the same task as a theme flip. Selectors are
   * now O(1) lookups against this snapshot.
   */
  domainFlags: Record<string, DomainFlags>;
  /** Recomputes `domainFlags` + `hasChanges` from current state. Call once per
   *  mutation, after the state it describes has been written. */
  recomputeDomainFlags: () => Record<string, DomainFlags>;
}

/**
 * Maps a settings scope (the IPC `domain`) to the settings card that owns it.
 * Single definition: the auto-save toast and the save-failure banner must
 * agree on which card a rejected key belongs to.
 */
export const SETTINGS_DOMAIN_TO_UI: Record<string, SettingsDomainId> = {
  persona: "persona",
  working_memory: "working_memory",
  personal_memory: "personal_memory",
  appearance: "appearance",
  interaction: "interaction",
  dictation: "interaction",
  realtime: "models",
  vad: "models",
  stt: "models",
  llm: "models",
  tts: "models",
  system: "models",
};

let appearanceDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let appearanceDebounceGeneration = 0;
let settingsAutoSaveTimer: ReturnType<typeof setTimeout> | null = null;
let autoSaveToastTimer: ReturnType<typeof setTimeout> | null = null;
let saveFailureTimers = new Map<string, ReturnType<typeof setTimeout>>();

/** In-flight `list_llm_models` promises keyed by fetch key, so the two
 * permanently mounted probing hooks share one request per endpoint. */
const remoteModelsInflight = new Map<string, Promise<LlmModelInfo[]>>();

let restartPollInterval: ReturnType<typeof setInterval> | null = null;

/** Poll cadence for mirroring the backend's restart progress. */
const RESTART_POLL_INTERVAL_MS = 500;
/** Upper bound so a failed restart cannot wedge the UI in "restarting" (30 * 500ms = 15s). */
const RESTART_POLL_MAX_TICKS = 30;

/** The six card domains that own a commit footer. */
export const COMMIT_DOMAIN_IDS = [
  "models",
  "persona",
  "working_memory",
  "personal_memory",
  "appearance",
  "interaction",
] as const;

/**
 * Catalog-affecting scopes for `commitChanges`. The catalog is manifest +
 * voices + model parameters + the TTS `active_voice_id`; `llm`/`tts`
 * inventory, selections, and keys never appear in it (see the `touchedKeys`
 * check at the reload site). `restoreDefaults` always reloads separately.
 */
const MODEL_CATALOG_SCOPES: ReadonlySet<string> = new Set([
  "vad",
  "stt",
  "system",
]);

/** Checks if a setting key requires an engine restart.
 * @deprecated Use `settingsContractService.isRestartKey(scope, key)` or backend contract reload policies.
 */
export function isRestartKey(scope: string, key: string): boolean {
  return settingsContractService.isRestartKey(scope, key);
}

/** Empty flag set used before any settings are loaded. */
const EMPTY_DOMAIN_FLAGS: Record<string, DomainFlags> = {};

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  draftSettings: null,
  modelCatalog: null,
  remoteModels: [],
  loadingRemoteModels: false,
  remoteModelsError: null,
  remoteModelsFetchedKey: null,
  capabilitiesCache: {},
  capabilitiesCacheError: null,
  isLoading: true,
  hasChanges: false,
  restartKeys: [],
  restartInFlight: false,
  error: null,
  contract: null,
  domainFlags: EMPTY_DOMAIN_FLAGS,

  loadContract: async () => {
    try {
      const contract = await settingsContractService.getSettingsContract();
      if (contract) {
        updateCloudProvidersFromContract(contract.llm_cloud_providers);
        updateTtsProvidersFromContract(contract.tts_providers);
        set({ contract });
      }
      return contract;
    } catch (err) {
      console.warn("[SettingsStore] Failed to load settings contract:", err);
      return null;
    }
  },

  /**
   * Recomputes the per-domain dirty/restart snapshot from the CURRENT state.
   * Called once per mutation, never inside a `set()` updater — reading the
   * store from an updater would be a side effect in a pure callback.
   */
  recomputeDomainFlags: () => {
    const flags: Record<string, DomainFlags> = {};
    for (const id of COMMIT_DOMAIN_IDS) {
      flags[id] = {
        dirty: get().isDomainDirty(id),
        requiresRestart: get().isDomainRequiringRestart(id),
      };
    }
    const hasChanges = COMMIT_DOMAIN_IDS.some((id) => flags[id].dirty);
    set({ domainFlags: flags, hasChanges });
    return flags;
  },

  loadSettings: async () => {
    try {
      get().loadContract().catch((err) => {
        console.warn("[SettingsStore] Non-blocking contract load error:", err);
      });

      const bootState = await getSettings();
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);

      set((state) => {
        if (state.draftSettings && state.draftSettings.appearance && cloned.appearance) {
          cloned.appearance.theme = state.draftSettings.appearance.theme;
          cloned.appearance.accent_seed = state.draftSettings.appearance.accent_seed;
        }
        return {
          settings: fetched,
          draftSettings: state.hasChanges ? state.draftSettings : cloned,
          isLoading: false,
          hasChanges: state.hasChanges,
          error: null,
        };
      });
      get().recomputeDomainFlags();
      applyTheme(fetched.appearance, { animate: false });
    } catch (err) {
      const reason = ipcErrorMessage(err);
      console.error("Failed to load settings:", err);
      set({ isLoading: false, error: reason || "Failed to load settings" });
    }
  },

  loadModelCatalog: async () => {
    try {
      const catalog = await requestModelCatalog();
      set({ modelCatalog: catalog, error: null });
    } catch (err) {
      const reason = ipcErrorMessage(err);
      console.error("Failed to load model catalog:", err);
      set({ error: reason || "Failed to load model catalog" });
    }
  },

  loadRemoteModels: async (providerConfig?: LlmProviderConfig, force = false) => {
    const draft = get().draftSettings || get().settings;
    const activeSel = draft?.llm?.active;
    const active = activeSel?.category ?? "embedded";
    let provider = providerConfig;
    if (!provider && draft && active !== "embedded") {
      if (active === "server") {
        const entry = draft.providers?.llm?.servers?.find((s) => s.id === activeSel?.provider);
        if (entry) {
          provider = {
            kind: "server",
            provider_id: entry.id,
            base_url: entry.url,
            model: activeSel?.model || "",
            api_key: entry.api_key || undefined,
            provider_name: entry.name || undefined,
          };
        }
      } else if (active === "cloud") {
        const cloudKeys = draft.llm?.cloud_keys ?? draft.providers?.llm?.cloud_keys;
        provider = {
          kind: "cloud",
          provider_id: activeSel?.provider ?? undefined,
          base_url: "",
          model: activeSel?.model || "",
          api_key: (activeSel?.provider && cloudKeys?.[activeSel.provider]) || undefined,
          provider_name: activeSel?.provider ?? undefined,
        };
      }
    }
    if (!provider || provider.kind === "embedded" || !provider.base_url) {
      if (get().remoteModelsError) set({ remoteModelsError: null, loadingRemoteModels: false });
      return;
    }
    // Cloud providers require an API key to list models. If the key is not yet configured,
    // wait for the user to add and save their key — do not probe and fail.
    if (provider.kind === "cloud" && !provider.api_key?.trim()) {
      if (get().remoteModelsError) set({ remoteModelsError: null, loadingRemoteModels: false });
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
        // The owning request records the error state; followers just rejoin.
      }
      return;
    }
    console.info(
      `[Providers] loadRemoteModels kind=${provider.kind} url=${provider.base_url} hasKey=${Boolean(provider.api_key)} force=${force}`
    );
    set({ loadingRemoteModels: true, remoteModelsError: null });
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
      set({ loadingRemoteModels: false, remoteModelsError: msg });
    } finally {
      if (remoteModelsInflight.get(fetchKey) === request) {
        remoteModelsInflight.delete(fetchKey);
      }
    }
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
      if (res.cache_error) {
        console.error("Capabilities cache unreadable:", res.cache_error);
      }
    } catch (err) {
      const msg = ipcErrorMessage(err);
      console.error("Failed to load capabilities cache:", err);
      set({ capabilitiesCacheError: msg });
    }
  },

  patchRemoteModelCapabilities: (modelId, caps) => {
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

  lastSavedTimestamp: 0,
  autoSavedDomain: null as string | null,
  failedSaveDomains: {} as Record<string, string[]>,
  deskFooter: null as { domain: string; title: string; detail: string } | null,
  setDeskFooter: (entry) => {
    const cur = get().deskFooter;
    if (
      cur?.domain === entry?.domain &&
      cur?.title === entry?.title &&
      cur?.detail === entry?.detail
    ) {
      return;
    }
    set({ deskFooter: entry });
  },
  triggerSaveFailure: (domainId: string, keys: string[]) => {
    set((state) => ({
      failedSaveDomains: { ...state.failedSaveDomains, [domainId]: keys },
    }));
    // Re-arming without clearing the old timer let an earlier failure's timer
    // clear a newer failure's banner early.
    const existing = saveFailureTimers.get(domainId);
    if (existing) clearTimeout(existing);
    saveFailureTimers.set(
      domainId,
      setTimeout(() => {
        saveFailureTimers.delete(domainId);
        set((state) => {
          if (!state.failedSaveDomains[domainId]) return {};
          const next = { ...state.failedSaveDomains };
          delete next[domainId];
          return { failedSaveDomains: next };
        });
      }, 6000)
    );
  },
  triggerAutoSaveToast: (domainId: string) => {
    set({ autoSavedDomain: domainId, lastSavedTimestamp: Date.now() });
    // Same re-arm bug as above: toggling the theme twice inside 1800ms had the
    // first timer clear the second toast 800ms early, so the "Changes Saved"
    // footer visibly truncated on rapid edits.
    if (autoSaveToastTimer) clearTimeout(autoSaveToastTimer);
    autoSaveToastTimer = setTimeout(() => {
      autoSaveToastTimer = null;
      set((state) =>
        state.autoSavedDomain === domainId ? { autoSavedDomain: null } : {}
      );
    }, 1800);
  },

  updateDraft: <D extends keyof VoxSettings>(
    domain: D,
    key: DraftKeyMap[D],
    value: unknown,
    explicitDomainId?: SettingsDomainId
  ) => {
    const { settings, draftSettings } = get();
    if (!draftSettings || !settings) return;

    const currentVal = scopeEntries(draftSettings[domain])[key];
    if (JSON.stringify(currentVal) === JSON.stringify(value)) return;

    const newDraft = {
      ...draftSettings,
      [domain]: {
        ...scopeEntries(draftSettings[domain]),
        [key]: value,
      },
    };

    if (domain === "appearance" && (key === "theme" || key === "accent_seed")) {
      applyTheme(newDraft.appearance);

      if (appearanceDebounceTimer) {
        clearTimeout(appearanceDebounceTimer);
      }
      // Generation stamp: a second edit while the first IPC is in flight must
      // not have the older response overwrite `settings` with a stale value.
      // The old code nulled the timer handle inside the fired callback, so a
      // concurrent update started an unguarded second timer, and a late
      // resolution left the card permanently dirty.
      const generation = ++appearanceDebounceGeneration;
      appearanceDebounceTimer = setTimeout(() => {
        appearanceDebounceTimer = null;
        updateSetting("appearance", key, value)
          .then(() => {
            if (generation !== appearanceDebounceGeneration) return;
            const curSettings = get().settings;
            if (curSettings) {
              set({
                settings: {
                  ...curSettings,
                  appearance: {
                    ...curSettings.appearance,
                    [key]: value,
                  },
                },
              });
            }
            get().triggerAutoSaveToast("appearance");
          })
          .catch(console.error);
      }, 200);

      // Derived flags first, then a single write, so subscribers see the draft
      // and its dirty state in one render rather than two.
      set({ draftSettings: newDraft });
      get().recomputeDomainFlags();
      return;
    }

    const clearsRemoteError = key === "active" && Boolean(get().remoteModelsError);
    set({
      draftSettings: newDraft,
      ...(clearsRemoteError ? { remoteModelsError: null } : {}),
    });
    const flags = get().recomputeDomainFlags();

    if (!flags || !COMMIT_DOMAIN_IDS.some((d) => flags[d]?.dirty)) {
      if (settingsAutoSaveTimer) {
        clearTimeout(settingsAutoSaveTimer);
        settingsAutoSaveTimer = null;
      }
      return;
    }

    const targetDomainId = explicitDomainId || SETTINGS_DOMAIN_TO_UI[domain as string] || "models";
    const needsRestart = flags[targetDomainId]?.requiresRestart ?? false;

    // If this change or domain requires an engine restart, do NOT auto-commit.
    // Leave the card dirty with "Apply & Restart" button visible.
    if (needsRestart) {
      if (settingsAutoSaveTimer) {
        clearTimeout(settingsAutoSaveTimer);
        settingsAutoSaveTimer = null;
      }
      return;
    }

    // Hot or WorkerCommand: Automatically commit with 600ms debounce and flash "Saved" toast on that specific card
    if (settingsAutoSaveTimer) {
      clearTimeout(settingsAutoSaveTimer);
    }
    settingsAutoSaveTimer = setTimeout(() => {
      settingsAutoSaveTimer = null;
      const anyNeedsRestart = COMMIT_DOMAIN_IDS.some(
        (d) => get().domainFlags[d]?.requiresRestart
      );
      if (anyNeedsRestart) {
        return;
      }
      get()
        .commitChanges()
        .then(() => {
          get().triggerAutoSaveToast(targetDomainId);
        })
        .catch(console.error);
    }, 600);
  },

  isDomainDirty: (domainId: string) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return false;

    const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
    if (!dirtyKeys || dirtyKeys.length === 0) return false;

    for (const rule of dirtyKeys) {
      const scope = rule.scope as keyof VoxSettings;
      const draftScope = scopeEntries(draftSettings[scope]);
      const savedScope = scopeEntries(settings[scope]);

      if (!draftScope && !savedScope) continue;
      if (!draftScope || !savedScope) return true;

      if (rule.keys && rule.keys.length > 0) {
        for (const k of rule.keys) {
          const draftVal = draftScope[k];
          const savedVal = savedScope[k];
          if (JSON.stringify(draftVal) !== JSON.stringify(savedVal)) {
            return true;
          }
        }
      } else {
        if (JSON.stringify(draftScope) !== JSON.stringify(savedScope)) {
          return true;
        }
      }
    }

    return false;
  },

  isDomainRequiringRestart: (domainId: string) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return false;

    const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
    if (!dirtyKeys || dirtyKeys.length === 0) return false;

    for (const rule of dirtyKeys) {
      const scope = rule.scope as keyof VoxSettings;
      const draftScope = scopeEntries(draftSettings[scope]);
      const savedScope = scopeEntries(settings[scope]);

      if (!draftScope || !savedScope) continue;

      const keysToCheck = rule.keys || Object.keys(draftScope);
      for (const k of keysToCheck) {
        if (isRestartKey(scope, k)) {
          const draftVal = draftScope[k];
          const savedVal = savedScope[k];
          if (JSON.stringify(draftVal) !== JSON.stringify(savedVal)) {
            return true;
          }
        }
      }
    }

    return false;
  },

  isCategoryDirty: (category: string) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return false;

    const scope = category.toLowerCase() as keyof VoxSettings;
    const draftScope = scopeEntries(draftSettings?.[scope]);
    const savedScope = scopeEntries(settings?.[scope]);

    if (!draftScope && !savedScope) return false;
    if (!draftScope || !savedScope) return true;

    const contractKeys = settingsContractService.getSupportedKeys(scope);
    const keys = contractKeys && contractKeys.length > 0 ? contractKeys : SETTINGS_SCOPE_KEYS[scope as SettingsScope];
    if (keys && keys.length > 0) {
      for (const k of keys) {
        const draftVal = draftScope[k];
        const savedVal = savedScope[k];
        if (JSON.stringify(draftVal) !== JSON.stringify(savedVal)) {
          return true;
        }
      }
      return false;
    }
    return JSON.stringify(draftScope) !== JSON.stringify(savedScope);
  },

  discardCategoryChanges: (category: string) => {
    const { settings, updateDraft } = get();
    if (!settings) return;

    const scope = category.toLowerCase() as keyof VoxSettings;
    const savedScope = scopeEntries(settings?.[scope]);
    if (!savedScope) return;

    const contractKeys = settingsContractService.getSupportedKeys(scope);
    const keys = contractKeys && contractKeys.length > 0 ? contractKeys : SETTINGS_SCOPE_KEYS[scope as SettingsScope];
    if (keys && keys.length > 0) {
      keys.forEach((k) => updateDraft(scope, k as DraftKeyMap[typeof scope], savedScope[k]));
    } else {
      Object.keys(savedScope).forEach((k) => updateDraft(scope, k as DraftKeyMap[typeof scope], savedScope[k]));
    }
  },

  discardDomainChanges: (domainId: string) => {
    if (settingsAutoSaveTimer) {
      clearTimeout(settingsAutoSaveTimer);
      settingsAutoSaveTimer = null;
    }

    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
    if (!dirtyKeys) return;

    const newDraft = structuredClone(draftSettings);

    for (const rule of dirtyKeys) {
      const scope = rule.scope as keyof VoxSettings;
      const savedScope = scopeEntries(settings[scope]);
      if (savedScope) {
        if (!newDraft[scope]) {
          (newDraft as unknown as Record<string, Record<string, unknown>>)[scope] = {};
        }
        if (rule.keys) {
          rule.keys.forEach((k) => {
            (newDraft as unknown as Record<string, Record<string, unknown>>)[scope][k] = structuredClone(savedScope[k]);
          });
        } else {
          (newDraft as unknown as Record<string, unknown>)[scope] = structuredClone(savedScope);
        }
      }
    }

    if (domainId === "appearance") {
      applyTheme(settings.appearance);
    }

    set({ draftSettings: newDraft, autoSavedDomain: null });
    get().recomputeDomainFlags();
  },

  isCommitting: false,
  commitChanges: async (only) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    set({ isCommitting: true });
    const promises: Promise<unknown>[] = [];
    const restartKeys: string[] = [];
    const failures: { domain: string; key: string; reason: string }[] = [];
    const touchedScopes = new Set<string>();
    const touchedKeys = new Set<string>();
    let restartScheduled = false;

    const canonicalDomains: (keyof VoxSettings)[] = [
      "vad",
      "stt",
      "llm",
      "tts",
      "realtime",
      "interaction",
      "dictation",
      "working_memory",
      "appearance",
      "personal_memory",
      "persona",
      "system",
    ];

    for (const domain of canonicalDomains) {
      const draftObj = draftSettings[domain];
      const savedObj = settings[domain];
      if (!draftObj || typeof draftObj !== "object") continue;

      const savedMap = new Map<string, unknown>(
        savedObj && typeof savedObj === "object" ? Object.entries(savedObj) : []
      );

      for (const [key, val] of Object.entries(draftObj)) {
        const oldVal = savedMap.get(key);

        if (JSON.stringify(val) === JSON.stringify(oldVal)) continue;
        if (only && (domain !== only.domain || key !== only.key)) continue;

        touchedScopes.add(domain);
        touchedKeys.add(`${domain}.${key}`);
        promises.push(
          updateSetting(domain, key, val).then(
            (res) => {
              // The backend is the only authority on reload policy. It reports
              // the class per key and, for `Restart`, has already taken
              // responsibility for rebuilding the engine — so nothing here
              // needs to decide whether to reload.
              if (res?.reload_policy === "restart") {
                restartKeys.push(`${domain}.${key}`);
              }
              if (res?.restart_scheduled) {
                restartScheduled = true;
              }
            },
            (err: unknown) => {
              // The backend rejects unknown or invalid keys. Recording the
              // failure instead of swallowing it is what stops a silently
              // dropped setting from looking like a successful save.
              const reason = ipcErrorMessage(err);
              failures.push({ domain, key, reason });
              console.error(`[Settings] Failed to persist ${domain}.${key}:`, err);
            }
          )
        );
      }
    }

    try {
      await Promise.all(promises);

      if (failures.length > 0) {
        // Re-read from the backend so the draft reflects what was actually
        // stored, but keep ONLY the rejected keys from the pre-commit draft so
        // they can be retried. Every other key landed and must not linger as
        // dirty — and unrelated edits must never be discarded with them.
        const bootState = await getSettings();
        const fetched = bootState.settings;
        const merged = structuredClone(fetched);
        for (const f of failures) {
          const draftScope = scopeEntries(draftSettings[f.domain as keyof VoxSettings]);
          const mergedScope = scopeEntries(merged[f.domain as keyof VoxSettings]);
          if (draftScope && mergedScope && f.key in draftScope) {
            mergedScope[f.key] = structuredClone(draftScope[f.key]);
          }
        }
        set({
          settings: fetched,
          draftSettings: merged as VoxSettings,
          hasChanges: true,
          isLoading: false,
        });
        get().recomputeDomainFlags();
        const failedByUiDomain = new Map<string, string[]>();
        for (const f of failures) {
          const uiDomain = SETTINGS_DOMAIN_TO_UI[f.domain] ?? "models";
          const keys = failedByUiDomain.get(uiDomain);
          if (keys) keys.push(f.key);
          else failedByUiDomain.set(uiDomain, [f.key]);
        }
        for (const [uiDomain, keys] of failedByUiDomain) {
          get().triggerSaveFailure(uiDomain, keys);
        }
        return;
      }

      set({ hasChanges: false });
      const bootState = await getSettings();
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);
      if (only) {
        // A scoped commit must not discard other dirty keys: restore every
        // pre-commit draft value that differs from the fresh snapshot except
        // the key just committed.
        const preDraft = draftSettings;
        for (const domain of canonicalDomains) {
          const preScope = scopeEntries(preDraft[domain]);
          const savedScope = scopeEntries(settings[domain]);
          const clonedScope = scopeEntries(cloned[domain]);
          if (!preScope || !clonedScope) continue;
          for (const key of Object.keys(preScope)) {
            if (domain === only.domain && key === only.key) continue;
            if (JSON.stringify(preScope[key]) !== JSON.stringify(savedScope?.[key])) {
              clonedScope[key] = structuredClone(preScope[key]);
            }
          }
        }
      }
      set({ settings: fetched, draftSettings: cloned, hasChanges: false, isLoading: false });
      get().recomputeDomainFlags();

      // Only re-read the catalog when a committed key actually feeds it. The
      // catalog is manifest + voices + model parameters + the TTS
      // active_voice_id; provider inventory, selections, keys, and prompts
      // never appear in it, so reloading for those is a full manifest
      // re-parse plus database round trips that remount the catalog views
      // (the visible flicker after every server save).
      const catalogAffected =
        touchedKeys.has("tts.active_voice_id") ||
        [...touchedScopes].some((scope) => MODEL_CATALOG_SCOPES.has(scope));
      if (catalogAffected) {
        await get().loadModelCatalog().catch(console.error);
      }

      // Replace wholesale rather than merging: a key that stopped being
      // restart-classified must not linger in the banner on the next commit.
      set({ restartKeys, restartInFlight: restartScheduled });
      if (restartScheduled) {
        // The restart is asynchronous and coalesced. Poll until the backend's
        // runner has finished so the "restarting" state cannot get stuck on.
        get().trackRestartCompletion();
      }
    } finally {
      set({ isCommitting: false });
    }
  },

  flushPendingAutosave: () => {
    if (!settingsAutoSaveTimer) return;
    clearTimeout(settingsAutoSaveTimer);
    settingsAutoSaveTimer = null;
    const anyNeedsRestart = COMMIT_DOMAIN_IDS.some(
      (d) => get().domainFlags[d]?.requiresRestart
    );
    if (anyNeedsRestart) {
      return;
    }
    // No toast here: the card is closing, so there is nothing to flash it on.
    // The persisted values are visible when the card reopens.
    get()
      .commitChanges()
      .catch(console.error);
  },

  /**
   * Mirrors the backend's `restart_in_flight` / `restart_requested` flag into the store.
   *
   * Polls `getRestartStatus()` every 500ms while restarting is in flight. Clears
   * `restartInFlight` as soon as the engine has restarted or after a 15-second
   * safety timeout so a failed restart cannot wedge the UI in "restarting".
   */
  trackRestartCompletion: () => {
    if (restartPollInterval) {
      clearInterval(restartPollInterval);
      restartPollInterval = null;
    }

    let ticks = 0;
    restartPollInterval = setInterval(async () => {
      ticks += 1;
      if (ticks > RESTART_POLL_MAX_TICKS || !get().restartInFlight) {
        if (restartPollInterval) {
          clearInterval(restartPollInterval);
          restartPollInterval = null;
        }
        if (get().restartInFlight) {
          set({ restartInFlight: false });
        }
        return;
      }

      try {
        const status = await getRestartStatus();
        if (!status.restarting) {
          if (restartPollInterval) {
            clearInterval(restartPollInterval);
            restartPollInterval = null;
          }
          if (get().restartInFlight) {
            set({ restartInFlight: false });
          }
        }
      } catch (err) {
        console.warn("[SettingsStore] Failed to poll restart status:", err);
      }
    }, RESTART_POLL_INTERVAL_MS);
  },

  discardChanges: () => {
    if (settingsAutoSaveTimer) {
      clearTimeout(settingsAutoSaveTimer);
      settingsAutoSaveTimer = null;
    }
    const { settings } = get();
    if (!settings) return;
    const cloned = structuredClone(settings);
    applyTheme(settings.appearance);
    set({ draftSettings: cloned, hasChanges: false, autoSavedDomain: null });
    get().recomputeDomainFlags();
  },

  restoreDefaults: async () => {
    try {
      const res = await resetSettings();
      const defaults = res.settings;
      const cloned = structuredClone(defaults);
      applyTheme(defaults.appearance);
      // `restart_scheduled` is the backend's own verdict; `["all"]` is only a
      // display placeholder for a reset that touched every domain.
      const restartKeys = res.reload_policy === "restart" ? ["all"] : [];
      set({
        settings: defaults,
        draftSettings: cloned,
        hasChanges: false,
        restartKeys,
        restartInFlight: res.restart_scheduled,
      });
      get().recomputeDomainFlags();
      // A reset restores every default, so the catalog is always affected.
      await get().loadModelCatalog().catch(console.error);
      if (res.restart_scheduled) {
        get().trackRestartCompletion();
      }
    } catch (err) {
      console.error("Failed to restore defaults:", err);
    }
  },

  toggleTheme: () => {
    const { draftSettings } = get();
    if (!draftSettings?.appearance) return;
    const currentTheme = draftSettings.appearance.theme;
    const newTheme = currentTheme === "dark" ? "light" : "dark";
    get().updateDraft("appearance", "theme", newTheme);
  },

  clearRestartKeys: () => {
    if (restartPollInterval) {
      clearInterval(restartPollInterval);
      restartPollInterval = null;
    }
    set({ restartKeys: [], restartInFlight: false });
  },
}));
