import { create } from "zustand";
import {
  requestBootState,
  requestModelCatalog,
  updateSetting,
  updateInteractionMode,
  resetSettings,
  getCachedCapabilities,
} from "@/services/settingsService";
import { hexToRgb } from "@/shared/lib/utils";
import { DOMAIN_DIRTY_KEYS, type SettingsDomainId } from "@/data/settingsCopy";

export type PipelineMode = "modular" | "realtime";
export type LlmActiveProvider = "embedded" | "server" | "cloud";
export type SttActiveProvider = "embedded" | "cloud";
export type TtsActiveProvider = "edge_tts" | "supertonic" | "chatterbox" | "chatterbox_remote";
export type RealtimeActiveProvider =
  | "gemini_live"
  | "openai_realtime"
  | "deepgram_voice_agent"
  | "elevenlabs_convai";

export type LlmProviderKind = "embedded" | "open_ai_compat";

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

export type TtsProviderConfig =
  | { kind: "supertonic" }
  | { kind: "chatterbox"; language: string; quality_steps: number; speed: number }
  | {
      kind: "chatterbox_remote";
      endpoint: string;
      language: string;
      quality_steps: number;
      speed: number;
      remote_path: string;
    }
  | { kind: "edge_tts"; voice?: string };

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
  files?: ModelEntry[];
}

// Alias for backwards compatibility during component migration
export type ModelMetadata = ModelGroupInfo;

export interface VoiceProfile {
  id: number;
  name: string;
}

export interface ModelCatalog {
  llm: ModelGroupInfo[];
  asr: ModelGroupInfo[];
  tts: ModelGroupInfo[];
  vad: ModelGroupInfo[];
  auxiliary: ModelGroupInfo[];
  model_groups: ModelGroupInfo[];
  voices: VoiceProfile[];
  preset_colors: string[];
}

export interface AudioSettings {
  output_mode: string;
  input_device: string | null;
}

export interface VadSettings {
  threshold: number;
  ptt_noise_gate: number;
  backend: "earshot" | "ten_vad";
  vad_backend?: "earshot" | "ten_vad";
}

export interface SttEmbeddedConfig {
  model: string;
}

export interface SttCloudConfig {
  provider: string;
  model: string;
  language: string;
  region?: string | null;
  endpoint?: string | null;
  api_key?: string | null;
}

export interface SttSettings {
  active: SttActiveProvider;
  transliterate_enabled: boolean;
  embedded: SttEmbeddedConfig;
  cloud: SttCloudConfig;
  model?: string;
  provider?: SttProviderConfig;
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
  embedded: LlmEmbeddedConfig;
  server: LlmRemoteConfig;
  cloud: LlmRemoteConfig;
}

export interface TtsEdgeTtsConfig {
  voice: string;
}

export interface TtsSupertonicConfig {}

export interface TtsChatterboxConfig {
  language: string;
}

export interface TtsChatterboxRemoteConfig {
  endpoint: string;
  language: string;
  remote_path: string;
}

export interface TtsSettings {
  active: TtsActiveProvider;
  voice_index: number;
  quality_steps: number;
  speed: number;
  edge_tts: TtsEdgeTtsConfig;
  supertonic: TtsSupertonicConfig;
  chatterbox: TtsChatterboxConfig;
  chatterbox_remote: TtsChatterboxRemoteConfig;
  provider?: TtsProviderConfig;
  voice?: number;
}

export interface GeminiLiveConfig {
  api_key: string;
  model: string;
  voice_name: string;
  language_code: string;
  temperature: number;
  enable_web_search: boolean;
  resume_handle: string | null;
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
  gemini_live: GeminiLiveConfig;
  openai_realtime: OpenAiRealtimeConfig;
  deepgram_voice_agent: DeepgramVoiceAgentConfig;
  elevenlabs_convai: ElevenLabsConvaiConfig;
  provider?: RealtimeActiveProvider;
  gemini?: GeminiLiveConfig;
  openai?: OpenAiRealtimeConfig;
  deepgram?: DeepgramVoiceAgentConfig;
  elevenlabs?: ElevenLabsConvaiConfig;
}

export interface InteractionSettings {
  mode: "Passive" | "PTT";
  main_app_mode?: "Passive" | "PTT";
  auto_sleep_timeout: number;
  pipeline_mode: PipelineMode;
}

export interface DictationSettings {
  enabled: boolean;
  interaction_mode: "passive" | "ptt";
  hotkey: string;
  output_mode: "paste" | "clipboard" | "tray";
}

export interface HistorySettings {
  private_mode: boolean;
  tray_history_limit: number;
}

export interface AppearanceSettings {
  theme: string;
  accent_seed: string;
}

export interface MemorySettings {
  context_retrieval_enabled: boolean;
  pipeline_processing_enabled: boolean;
  max_context_share: number;
  context_chaining_window_hours: number;
  top_k_facts: number;
  max_hops: number;
  semantic_similarity_cutoff: number;
}

export interface PersonaSettings {
  modular_prompt: string;
  realtime_prompt: string;
}

export interface SystemSettings {
  log_level: string;
  telemetry_enabled: boolean;
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
  history: HistorySettings;
  appearance: AppearanceSettings;
  memory: MemorySettings;
  persona: PersonaSettings;
  system: SystemSettings;
}

interface SettingsState {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  modelCatalog: ModelCatalog | null;
  capabilitiesCache: Record<string, ModelCapabilities>;
  isLoading: boolean;
  hasChanges: boolean;
  restartKeys: string[];
  error: string | null;

  loadSettings: () => Promise<void>;
  loadModelCatalog: () => Promise<void>;
  loadCapabilitiesCache: () => Promise<void>;
  updateDraft: (domain: keyof VoxSettings, key: string, value: any) => void;
  commitChanges: () => Promise<void>;
  discardChanges: () => void;
  isDomainDirty: (domainId: string) => boolean;
  discardDomainChanges: (domainId: string) => void;
  restoreDefaults: () => Promise<void>;
  toggleTheme: () => void;
  isCommitting: boolean;
  autoSavedDomain: string | null;
  lastSavedTimestamp: number;
  triggerAutoSaveToast: (domainId: string) => void;
}

function applyAppearance(appearance?: AppearanceSettings) {
  if (!appearance) return;
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", appearance.theme);
    document.documentElement.style.setProperty("--accent", hexToRgb(appearance.accent_seed));
    if (appearance.theme === "light") {
      document.documentElement.classList.add("light");
      document.documentElement.classList.remove("dark");
    } else {
      document.documentElement.classList.add("dark");
      document.documentElement.classList.remove("light");
    }
  }
}

let appearanceDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let settingsAutoSaveTimer: ReturnType<typeof setTimeout> | null = null;

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  draftSettings: null,
  modelCatalog: null,
  capabilitiesCache: {},
  isLoading: true,
  hasChanges: false,
  restartKeys: [],
  error: null,

  loadSettings: async () => {
    try {
      const [bootState, capsCache] = await Promise.all([
        requestBootState(),
        getCachedCapabilities().catch(() => ({})),
      ]);
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
          capabilitiesCache: capsCache || {},
          isLoading: false,
          hasChanges: state.hasChanges,
          error: null,
        };
      });
      applyAppearance(fetched.appearance);
    } catch (err: any) {
      console.error("Failed to load settings:", err);
      set({ isLoading: false, error: err?.message || String(err) || "Failed to load settings" });
    }
  },

  loadModelCatalog: async () => {
    try {
      const catalog = await requestModelCatalog();
      set({ modelCatalog: catalog, error: null });
    } catch (err: any) {
      console.error("Failed to load model catalog:", err);
      set({ error: err?.message || String(err) || "Failed to load model catalog" });
    }
  },

  loadCapabilitiesCache: async () => {
    try {
      const cache = await getCachedCapabilities();
      set({ capabilitiesCache: cache || {} });
    } catch (err) {
      console.error("Failed to load capabilities cache:", err);
    }
  },

  lastSavedTimestamp: 0,
  autoSavedDomain: null as string | null,
  triggerAutoSaveToast: (domainId: string) => {
    set({ autoSavedDomain: domainId, lastSavedTimestamp: Date.now() });
    setTimeout(() => {
      set((state) => (state.autoSavedDomain === domainId ? { autoSavedDomain: null } : {}));
    }, 1800);
  },

  updateDraft: (domain: keyof VoxSettings, key: string, value: any) => {
    const { settings, draftSettings } = get();
    if (!draftSettings || !settings) return;

    const currentVal = (draftSettings[domain] as any)?.[key];
    if (JSON.stringify(currentVal) === JSON.stringify(value)) return;

    const newDraft = {
      ...draftSettings,
      [domain]: {
        ...(draftSettings[domain] as any),
        [key]: value,
      },
    };

    if (domain === "appearance" && (key === "theme" || key === "accent_seed")) {
      applyAppearance(newDraft.appearance);

      if (appearanceDebounceTimer) {
        clearTimeout(appearanceDebounceTimer);
      }
      appearanceDebounceTimer = setTimeout(() => {
        updateSetting("appearance", key, value).catch(console.error);
        appearanceDebounceTimer = null;
      }, 200);

      set({ draftSettings: newDraft });
      return;
    }

    set({ draftSettings: newDraft });
    const hasChanges = ["models", "history", "persona", "memory", "interaction"].some((d) =>
      get().isDomainDirty(d)
    );
    set({ hasChanges });

    // ─── Hybrid Auto-Save Logic ───
    // Check if the modified key requires a heavy restart
    const requiresRestart =
      (domain === "stt" && key === "embedded") ||
      (domain === "stt" && key === "active") ||
      (domain === "llm" && key === "embedded") ||
      (domain === "llm" && key === "active") ||
      (domain === "llm" && key === "context_window") ||
      (domain === "llm" && key === "threads") ||
      (domain === "tts" && key === "active") ||
      (domain === "vad" && key === "backend") ||
      (domain === "audio" && key === "input_device");

    if (!requiresRestart) {
      // Determine mapped SettingsDomainId for the toast
      const domainMap: Record<string, string> = {
        persona: "persona",
        memory: "memory",
        history: "history",
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
      const targetDomainId = domainMap[domain as string] || "models";

      // Hot or WorkerCommand: Automatically commit with 600ms debounce and flash "Saved" toast on that specific card
      if (settingsAutoSaveTimer) {
        clearTimeout(settingsAutoSaveTimer);
      }
      settingsAutoSaveTimer = setTimeout(() => {
        get()
          .commitChanges()
          .then(() => {
            get().triggerAutoSaveToast(targetDomainId);
          })
          .catch(console.error);
        settingsAutoSaveTimer = null;
      }, 600);
    }
  },

  isDomainDirty: (domainId: string) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return false;

    const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
    if (!dirtyKeys || dirtyKeys.length === 0) return false;

    for (const rule of dirtyKeys) {
      const scope = rule.scope as keyof VoxSettings;
      const draftScope = draftSettings[scope] as any;
      const savedScope = settings[scope] as any;

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

  discardDomainChanges: (domainId: string) => {
    const { settings, draftSettings, updateDraft } = get();
    if (!settings || !draftSettings) return;

    const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
    if (!dirtyKeys) return;

    for (const rule of dirtyKeys) {
      const scope = rule.scope as keyof VoxSettings;
      const savedScope = settings[scope] as any;
      if (savedScope) {
        if (rule.keys) {
          rule.keys.forEach((k) => updateDraft(scope, k, savedScope[k]));
        } else {
          Object.keys(savedScope).forEach((k) => updateDraft(scope, k, savedScope[k]));
        }
      }
    }
  },

  isCommitting: false,
  commitChanges: async () => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    set({ isCommitting: true });
    const promises: Promise<any>[] = [];
    const restartKeys: string[] = [];

    const canonicalDomains: (keyof VoxSettings)[] = [
      "audio",
      "vad",
      "stt",
      "llm",
      "tts",
      "realtime",
      "interaction",
      "dictation",
      "history",
      "appearance",
      "memory",
      "persona",
      "system",
    ];

    for (const domain of canonicalDomains) {
      const draftObj = (draftSettings as any)[domain];
      const savedObj = (settings as any)[domain];
      if (!draftObj) continue;

      for (const key in draftObj) {
        const val = draftObj[key];
        const oldVal = savedObj ? savedObj[key] : undefined;

        if (JSON.stringify(val) !== JSON.stringify(oldVal)) {
          if (domain === "interaction" && key === "mode") {
            promises.push(updateInteractionMode("main", val));
          } else {
            promises.push(
              updateSetting(domain, key, val).then((res: any) => {
                if (res?.reload_policy === "restart") {
                  restartKeys.push(`${domain}.${key}`);
                }
              })
            );
          }
        }
      }
    }

    try {
      await Promise.all(promises);
      set({ hasChanges: false });
      const bootState = await requestBootState();
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);
      set({ settings: fetched, draftSettings: cloned, hasChanges: false, isLoading: false });

      if (restartKeys.length > 0) {
        set({ restartKeys });
      }
    } finally {
      set({ isCommitting: false });
    }
  },

  discardChanges: () => {
    const { settings } = get();
    if (!settings) return;
    const cloned = structuredClone(settings);
    applyAppearance(settings.appearance);
    set({ draftSettings: cloned, hasChanges: false });
  },

  restoreDefaults: async () => {
    try {
      const defaults = await resetSettings();
      const cloned = structuredClone(defaults);
      applyAppearance(defaults.appearance);
      set({ settings: defaults, draftSettings: cloned, hasChanges: false });
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
    set({ restartKeys: [] });
  },
}));
