import { create } from "zustand";
import {
  getSettings,
  requestModelCatalog,
  updateSetting,
  resetSettings,
  probeModelCapabilitiesFull,
} from "@/services/settingsService";
import { applyTheme } from "@/shared/theme";
import { DOMAIN_DIRTY_KEYS, SETTINGS_SCOPE_KEYS, type SettingsDomainId, type SettingsScope } from "@/data/settingsCopy";

/** Reads a settings scope as a key-value map for dynamic key access. The
 * single guarded cast in the store; every dynamic read funnels through here. */
function scopeEntries(scope: unknown): Record<string, unknown> {
  if (typeof scope !== "object" || scope === null) return {};
  return scope as Record<string, unknown>;
}

export type PipelineMode = "modular" | "realtime";
export type LlmActiveProvider = "embedded" | "server" | "cloud";
export type SttActiveProvider = "embedded" | "cloud";
export type TtsActiveProvider = "edge_tts" | "supertonic" | "kokoro" | "chatterbox" | "chatterbox_remote" | "zipvoice";
export type RealtimeActiveProvider =
  | "gemini_live"
  | "openai_realtime"
  | "deepgram_voice_agent"
  | "elevenlabs_convai";

export type LlmProviderKind = "embedded" | "server" | "cloud";

export interface LlmProviderConfig {
  kind: LlmProviderKind;
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

export interface ModelCapabilities {
  model_id: string;
  provider_kind: string;
  supports_tools: boolean;
  supports_latin: boolean;
  supports_devanagari: boolean;
  context_window?: number | null;
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
}

export type AudioOutputMode = "Speaker" | "Headset";

export interface AudioSettings {
  output_mode: AudioOutputMode;
  input_device: string | null;
}

export interface VadSettings {
  threshold: number;
  ptt_noise_gate: number;
  vad_backend: "earshot" | "ten_vad" | "silero_vad";
  silence_duration_ms: number;
  speech_onset_ms: number;
  max_speech_duration_s: number;
}

export interface SttEmbeddedConfig {
  model: string;
  partial_throttle_ms: number;
  threads: number;
}

export interface SttCloudConfig {
  provider: string;
  model: string;
  language: string;
  region: string;
  project_id?: string | null;
  endpoint?: string | null;
}

export interface SttSettings {
  active: SttActiveProvider;
  transliterate_enabled: boolean;
  embedded: SttEmbeddedConfig;
  cloud: SttCloudConfig;
}

export interface LlmEmbeddedConfig {
  model: string;
}

export interface LlmRemoteConfig {
  base_url: string;
  model: string;
  api_key?: string | null;
  provider_name?: string | null;
}

export interface LlmSettings {
  active: LlmActiveProvider;
  temperature: number;
  compaction_temperature: number;
  max_output_tokens: number;
  context_window: number;
  threads: number;
  reasoning_enabled: boolean;
  embedded: LlmEmbeddedConfig;
  server: LlmRemoteConfig;
  cloud: LlmRemoteConfig;
  cloud_keys: Record<string, string>;
}

export interface TtsEdgeTtsConfig {
  voice: string | null;
}

export interface TtsSupertonicConfig {}
export interface TtsKokoroConfig {}

export interface TtsChatterboxConfig {
  language: string;
  voice_id?: string | null;
}

export interface TtsChatterboxRemoteConfig {
  endpoint: string;
  language: string;
  remote_path: string;
  voice_id?: string | null;
}

export interface TtsZipvoiceConfig {
  voice_id?: string | null;
  guidance_scale: number;
}

export interface TtsSettings {
  active: TtsActiveProvider;
  voice_index: number;
  speed: number;
  threads: number;
  edge_tts: TtsEdgeTtsConfig;
  supertonic: TtsSupertonicConfig;
  kokoro: TtsKokoroConfig;
  chatterbox: TtsChatterboxConfig;
  chatterbox_remote: TtsChatterboxRemoteConfig;
  zipvoice: TtsZipvoiceConfig;
}

export interface GeminiRealtimeConfig {
  api_key: string;
  model: string;
  voice_name: string;
  language_code: string;
  temperature: number;
  enable_web_search: boolean;
}

export interface OpenAiRealtimeConfig {
  api_key: string;
  model: string;
  voice: string;
}

export interface DeepgramVoiceAgentConfig {
  api_key: string;
  model: string;
  voice: string;
  temperature: number;
  agent_mode: boolean;
}

export interface ElevenLabsConvaiConfig {
  api_key: string;
  agent_id: string;
}

export interface RealtimeSettings {
  active: RealtimeActiveProvider;
  gemini_live: GeminiRealtimeConfig;
  openai_realtime: OpenAiRealtimeConfig;
  deepgram_voice_agent: DeepgramVoiceAgentConfig;
  elevenlabs_convai: ElevenLabsConvaiConfig;
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

export interface VoxSettings {
  audio: AudioSettings;
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
}

export interface SettingsState {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  modelCatalog: ModelCatalog | null;
  capabilitiesCache: Record<string, ModelCapabilities>;
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

  loadSettings: () => Promise<void>;
  loadModelCatalog: () => Promise<void>;
  loadCapabilitiesCache: () => Promise<void>;
  updateDraft: (
    domain: keyof VoxSettings,
    key: string,
    value: unknown,
    explicitDomainId?: SettingsDomainId
  ) => void;
  commitChanges: () => Promise<void>;
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
  /** Domains whose most recent commit was rejected by the backend. */
  failedSaveDomains: Record<string, true>;
  failedSaveKeys: string[];
  triggerSaveFailure: (domainId: string, keys: string[]) => void;
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
  audio: "models",
  vad: "models",
  stt: "models",
  llm: "models",
  tts: "models",
  system: "models",
};

let appearanceDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let settingsAutoSaveTimer: ReturnType<typeof setTimeout> | null = null;

/** Poll cadence for mirroring the backend's restart progress. */
const RESTART_POLL_INTERVAL_MS = 400;
/** Upper bound so a failed restart cannot wedge the UI in "restarting". */
const RESTART_POLL_MAX_TICKS = 30;

/** Checks if a setting key requires an engine restart according to the backend policy table */
export function isRestartKey(scope: string, key: string): boolean {
  if (scope === "audio" && key === "input_device") return true;
  if (
    scope === "stt" &&
    ["active", "model", "provider", "embedded", "cloud", "threads"].includes(key)
  )
    return true;
  if (
    scope === "llm" &&
    [
      "active",
      "model",
      "provider",
      "server",
      "cloud",
      "cloud_keys",
      "embedded",
      "context_window",
      "threads",
    ].includes(key)
  )
    return true;
  if (
    scope === "tts" &&
    [
      "active",
      "provider",
      "threads",
      "edge_tts",
      "supertonic",
      "kokoro",
      "chatterbox",
      "chatterbox_remote",
      "zipvoice",
    ].includes(key)
  )
    return true;
  if (scope === "vad" && key === "vad_backend") return true;
  if (scope === "interaction" && key === "pipeline_mode") return true;
  if (scope === "dictation" && key === "hotkey") return true;
  if (scope === "realtime") return true;
  return false;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  draftSettings: null,
  modelCatalog: null,
  capabilitiesCache: {},
  isLoading: true,
  hasChanges: false,
  restartKeys: [],
  restartInFlight: false,
  error: null,

  loadSettings: async () => {
    try {
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
      applyTheme(fetched.appearance, { animate: false });
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      console.error("Failed to load settings:", err);
      set({ isLoading: false, error: reason || "Failed to load settings" });
    }
  },

  loadModelCatalog: async () => {
    try {
      const catalog = await requestModelCatalog();
      set({ modelCatalog: catalog, error: null });
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      console.error("Failed to load model catalog:", err);
      set({ error: reason || "Failed to load model catalog" });
    }
  },

  loadCapabilitiesCache: async () => {
    try {
      const res = await probeModelCapabilitiesFull();
      set({ capabilitiesCache: res.cached_map || {} });
    } catch (err) {
      console.error("Failed to load capabilities cache:", err);
    }
  },

  lastSavedTimestamp: 0,
  autoSavedDomain: null as string | null,
  failedSaveDomains: {} as Record<string, true>,
  failedSaveKeys: [] as string[],
  triggerSaveFailure: (domainId: string, keys: string[]) => {
    set((state) => ({
      failedSaveDomains: { ...state.failedSaveDomains, [domainId]: true },
      failedSaveKeys: keys,
    }));
    setTimeout(() => {
      set((state) => {
        if (!state.failedSaveDomains[domainId]) return {};
        const next = { ...state.failedSaveDomains };
        delete next[domainId];
        return { failedSaveDomains: next };
      });
    }, 6000);
  },
  triggerAutoSaveToast: (domainId: string) => {
    set({ autoSavedDomain: domainId, lastSavedTimestamp: Date.now() });
    setTimeout(() => {
      set((state) => (state.autoSavedDomain === domainId ? { autoSavedDomain: null } : {}));
    }, 1800);
  },

  updateDraft: (
    domain: keyof VoxSettings,
    key: string,
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
      appearanceDebounceTimer = setTimeout(() => {
        updateSetting("appearance", key, value)
          .then(() => {
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
        appearanceDebounceTimer = null;
      }, 200);

      set({ draftSettings: newDraft });
      return;
    }

    set({ draftSettings: newDraft });
    const hasChanges = [
      "models",
      "persona",
      "working_memory",
      "personal_memory",
      "appearance",
      "interaction",
    ].some((d) => get().isDomainDirty(d));
    set({ hasChanges });

    if (!hasChanges) {
      if (settingsAutoSaveTimer) {
        clearTimeout(settingsAutoSaveTimer);
        settingsAutoSaveTimer = null;
      }
      return;
    }

    const targetDomainId = explicitDomainId || SETTINGS_DOMAIN_TO_UI[domain as string] || "models";
    const needsRestart = get().isDomainRequiringRestart(targetDomainId);

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
      const anyNeedsRestart = [
        "models",
        "persona",
        "working_memory",
        "personal_memory",
        "appearance",
        "interaction",
      ].some((d) => get().isDomainRequiringRestart(d));
      if (anyNeedsRestart) {
        return;
      }
      get()
        .commitChanges()
        .then(() => {
          get().triggerAutoSaveToast(targetDomainId);
        })
        .catch(console.error);
      settingsAutoSaveTimer = null;
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

    const keys = SETTINGS_SCOPE_KEYS[scope as SettingsScope];
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

    const keys = SETTINGS_SCOPE_KEYS[scope as SettingsScope];
    if (keys && keys.length > 0) {
      keys.forEach((k: string) => updateDraft(scope, k, savedScope[k]));
    } else {
      Object.keys(savedScope).forEach((k: string) => updateDraft(scope, k, savedScope[k]));
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

    const hasChanges = [
      "models",
      "persona",
      "working_memory",
      "personal_memory",
      "appearance",
      "interaction",
    ].some((d) => get().isDomainDirty(d));
    set({ hasChanges });
  },

  isCommitting: false,
  commitChanges: async () => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    set({ isCommitting: true });
    const promises: Promise<unknown>[] = [];
    const restartKeys: string[] = [];
    const failures: { domain: string; key: string; reason: string }[] = [];
    let restartScheduled = false;

    const canonicalDomains: (keyof VoxSettings)[] = [
      "audio",
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
              const reason = err instanceof Error ? err.message : String(err);
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
        // stored. A rejected key must not linger in the draft, or the next
        // commit retries it forever and the UI keeps showing it as changed.
        const bootState = await getSettings();
        const fetched = bootState.settings;
        set({
          settings: fetched,
          draftSettings: structuredClone(fetched),
          hasChanges: false,
          isLoading: false,
        });
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
      set({ settings: fetched, draftSettings: cloned, hasChanges: false, isLoading: false });
      await get().loadModelCatalog().catch(console.error);

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
    const anyNeedsRestart = [
      "models",
      "persona",
      "working_memory",
      "personal_memory",
      "appearance",
      "interaction",
    ].some((d) => get().isDomainRequiringRestart(d));
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
   * Mirrors the backend's `restart_in_flight` flag into the store.
   *
   * The restart runs on a spawned task with no completion event, so this polls
   * the `SettingsUpdated` IPC event (re-emitted as the engine comes back) to
   * clear the flag. Bounded so a failed restart cannot wedge the UI in
   * "restarting" forever.
   */
  trackRestartCompletion: () => {
    let ticks = 0;
    const poll = setInterval(() => {
      ticks += 1;
      if (ticks > RESTART_POLL_MAX_TICKS || !get().restartInFlight) {
        clearInterval(poll);
        if (get().restartInFlight) {
          set({ restartInFlight: false });
        }
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
    set({ restartKeys: [], restartInFlight: false });
  },
}));
