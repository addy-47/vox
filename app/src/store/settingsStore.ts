import { create } from "zustand";
import {
  requestBootState,
  requestModelCatalog,
  updateSetting,
  updateInteractionMode,
  resetSettings,
} from "@/services/settingsService";
import { hexToRgb } from "@/shared/lib/utils";

export type PipelineMode = "modular" | "realtime";
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
  | { kind: "chatterbox_remote"; endpoint: string; language: string; quality_steps: number; speed: number; remote_path: string }
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

export interface VoxSettings {
  ui: {
    theme: string;
    accent_seed: string;
    tray_enabled: boolean;
    tray_blur_density: number;
    tray_glass_tint: boolean;
    tray_history_limit: number;
  };
  audio: {
    output_mode: string;
    input_device: string | null;
  };
  vad: {
    threshold: number;
    ptt_noise_gate: number;
    vad_backend: "earshot" | "ten_vad";
  };
  asr: {
    model: string;
    transliterate_enabled: boolean;
    provider: SttProviderConfig;
  };
  llm: {
    model: string;
    ctx_size: number;
    threads: number;
    provider: LlmProviderConfig;
  };
  tts: {
    provider: TtsProviderConfig;
    voice: number;
    quality_steps: number;
    speed: number;
  };
  interaction: {
    main_app_mode: "Passive" | "PTT";
    tray_mode: "Passive" | "PTT";
    auto_sleep_timeout: number;
    pipeline_mode: PipelineMode;
  };
  telemetry: {
    enabled: boolean;
    log_level: string;
  };
  persistence: {
    private_mode: boolean;
  };
  memory: {
    context_retrieval_enabled: boolean;
    pipeline_processing_enabled: boolean;
    max_personal_memory_share: number;
    context_chaining_window_hours: number;
    top_k_facts: number;
    max_hops: number;
    semantic_similarity_cutoff: number;
  };
  assistant: {
    modular_prompt: string;
    realtime_prompt: string;
  };
  setup: {
    completed: boolean;
  };
  realtime: {
    provider: "gemini_live" | "openai_realtime" | "deepgram_voice_agent" | "elevenlabs_convai";
    gemini: {
      api_key: string;
      model: string;
      voice_name: string;
      language_code: string;
      temperature: number;
      enable_web_search: boolean;
      resume_handle: string | null;
    };
    openai: {
      api_key: string;
      model: string;
      voice: string;
    };
    deepgram: {
      api_key: string;
      model: string;
      voice: string;
      temperature: number;
      agent_mode: boolean;
    };
    elevenlabs: {
      api_key: string;
      agent_id: string;
    };
  };
}

interface SettingsState {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  modelCatalog: ModelCatalog | null;
  isLoading: boolean;
  hasChanges: boolean;
  restartKeys: string[];
  error: string | null;

  loadSettings: () => Promise<void>;
  loadModelCatalog: () => Promise<void>;
  updateDraft: (domain: keyof VoxSettings, key: string, value: any) => void;
  commitChanges: () => Promise<void>;
  discardChanges: () => void;
  isDomainDirty: (domainId: string) => boolean;
  discardDomainChanges: (domainId: string) => void;
  restoreDefaults: () => Promise<void>;
  toggleTheme: () => void;
  clearRestartKeys: () => void;
}

function applyAppearance(ui: VoxSettings["ui"]) {
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", ui.theme);
    document.documentElement.style.setProperty("--accent", hexToRgb(ui.accent_seed));
    if (ui.theme === "light") {
      document.documentElement.classList.add("light");
      document.documentElement.classList.remove("dark");
    } else {
      document.documentElement.classList.add("dark");
      document.documentElement.classList.remove("light");
    }
  }
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  draftSettings: null,
  modelCatalog: null,
  isLoading: true,
  hasChanges: false,
  restartKeys: [],
  error: null,

  loadSettings: async () => {
    try {
      const bootState = await requestBootState();
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);
      
      set((state) => {
        if (state.draftSettings) {
          cloned.ui.theme = state.draftSettings.ui.theme;
          cloned.ui.accent_seed = state.draftSettings.ui.accent_seed;
        }
        return {
          settings: fetched,
          draftSettings: state.hasChanges ? state.draftSettings : cloned,
          isLoading: false,
          hasChanges: state.hasChanges,
          error: null,
        };
      });
      applyAppearance(fetched.ui);
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

  updateDraft: (domain, key, value) => {
    const { settings, draftSettings } = get();
    if (!draftSettings || !settings) return;

    const currentVal = (draftSettings[domain] as any)?.[key];
    if (currentVal === value) return;

    const newDraft = {
      ...draftSettings,
      [domain]: {
        ...(draftSettings[domain] as any),
        [key]: value,
      },
    };

    if (domain === "ui" && (key === "theme" || key === "accent_seed")) {
      applyAppearance(newDraft.ui);
      updateSetting(domain, key, value).catch(console.error);

      const newSettings = {
        ...settings,
        ui: {
          ...settings.ui,
          [key]: value,
        },
      };
      set({ settings: newSettings, draftSettings: newDraft, hasChanges: false });
      return;
    }

    const hasChanges = JSON.stringify(settings) !== JSON.stringify(newDraft);
    set({ draftSettings: newDraft, hasChanges });
  },

  isDomainDirty: (domainId: string) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return false;
    switch (domainId) {
      case "models": {
        const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
        if (isRealtime) {
          const provId = draftSettings.realtime?.provider || "gemini_live";
          const subkey = provId === "gemini_live" ? "gemini" :
                         provId === "openai_realtime" ? "openai" :
                         provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
                         
          const savedProvConfig = settings.realtime?.[subkey] || {};
          const draftProvConfig = draftSettings.realtime?.[subkey] || {};
          
          const { api_key: _, ...savedClean } = savedProvConfig;
          const { api_key: __, ...draftClean } = draftProvConfig;
          
          return JSON.stringify(savedClean) !== JSON.stringify(draftClean);
        }
        return (
          JSON.stringify(settings.vad) !== JSON.stringify(draftSettings.vad) ||
          JSON.stringify(settings.asr) !== JSON.stringify(draftSettings.asr) ||
          JSON.stringify(settings.tts) !== JSON.stringify(draftSettings.tts) ||
          JSON.stringify(settings.llm) !== JSON.stringify(draftSettings.llm)
        );
      }
      case "history":
        return (
          settings.persistence.private_mode !== draftSettings.persistence.private_mode ||
          settings.ui.tray_history_limit !== draftSettings.ui.tray_history_limit
        );
      case "persona":
        return JSON.stringify(settings.assistant) !== JSON.stringify(draftSettings.assistant);
      case "memory":
        return JSON.stringify(settings.memory) !== JSON.stringify(draftSettings.memory);
      case "appearance":
        return false;
      case "interaction": {
        const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
        let realtimeChanges = false;
        if (isRealtime) {
          if (settings.realtime?.provider !== draftSettings.realtime?.provider) {
            realtimeChanges = true;
          } else {
            const provId = draftSettings.realtime?.provider || "gemini_live";
            const subkey = provId === "gemini_live" ? "gemini" :
                           provId === "openai_realtime" ? "openai" :
                           provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
            if (settings.realtime?.[subkey]?.api_key !== draftSettings.realtime?.[subkey]?.api_key) {
              realtimeChanges = true;
            }
          }
        }
        
        return (
          settings.interaction.main_app_mode !== draftSettings.interaction.main_app_mode ||
          settings.interaction.auto_sleep_timeout !== draftSettings.interaction.auto_sleep_timeout ||
          settings.interaction.pipeline_mode !== draftSettings.interaction.pipeline_mode ||
          settings.interaction.tray_mode !== draftSettings.interaction.tray_mode ||
          realtimeChanges
        );
      }
      default:
        return false;
    }
  },

  discardDomainChanges: (domainId: string) => {
    const { settings, draftSettings, updateDraft } = get();
    if (!settings || !draftSettings) return;
    switch (domainId) {
      case "models": {
        const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
        if (isRealtime) {
          const provId = draftSettings.realtime?.provider || "gemini_live";
          const subkey = provId === "gemini_live" ? "gemini" :
                         provId === "openai_realtime" ? "openai" :
                         provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
                         
          const savedProvConfig = (settings.realtime as any)?.[subkey] || {};
          const currentDraftProvConfig = (draftSettings.realtime as any)?.[subkey] || {};
          
          const { api_key: _, ...savedClean } = savedProvConfig;
          updateDraft("realtime", subkey, {
            ...currentDraftProvConfig,
            ...savedClean
          });
        } else {
          Object.keys(settings.vad).forEach(k => updateDraft("vad", k, (settings.vad as any)[k]));
          Object.keys(settings.asr).forEach(k => updateDraft("asr", k, (settings.asr as any)[k]));
          updateDraft("llm", "model", settings.llm.model);
          updateDraft("llm", "ctx_size", settings.llm.ctx_size);
          updateDraft("llm", "threads", settings.llm.threads);
          Object.keys(settings.tts).forEach(k => updateDraft("tts", k, (settings.tts as any)[k]));
          if (settings.llm.provider && draftSettings?.llm.provider) {
            updateDraft("llm", "provider", {
              ...draftSettings.llm.provider,
              model: settings.llm.provider.model
            });
          }
        }
        break;
      }
      case "history":
        updateDraft("persistence", "private_mode", settings.persistence.private_mode);
        updateDraft("ui", "tray_history_limit", settings.ui.tray_history_limit);
        break;
      case "persona":
        Object.keys(settings.assistant).forEach(k => updateDraft("assistant", k, (settings.assistant as any)[k]));
        break;
      case "memory":
        Object.keys(settings.memory).forEach(k => updateDraft("memory", k, (settings.memory as any)[k]));
        break;
      case "appearance":
        updateDraft("ui", "theme", settings.ui.theme);
        updateDraft("ui", "accent_seed", settings.ui.accent_seed);
        break;
      case "interaction": {
        updateDraft("interaction", "main_app_mode", settings.interaction.main_app_mode);
        updateDraft("interaction", "auto_sleep_timeout", settings.interaction.auto_sleep_timeout);
        updateDraft("interaction", "pipeline_mode", settings.interaction.pipeline_mode);
        const currentDraftModel = draftSettings?.llm.provider?.model || "";
        updateDraft("llm", "provider", {
          ...settings.llm.provider,
          model: currentDraftModel
        });
        
        const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
        if (isRealtime) {
          updateDraft("realtime", "provider", settings.realtime.provider);
          const subkeys = ["gemini", "openai", "deepgram", "elevenlabs"] as const;
          subkeys.forEach(subkey => {
            if ((settings.realtime as any)?.[subkey] && (draftSettings?.realtime as any)?.[subkey]) {
              updateDraft("realtime", subkey, {
                ...(draftSettings.realtime as any)[subkey],
                api_key: (settings.realtime as any)[subkey].api_key
              });
            }
          });
        }
        break;
      }
    }
  },

  commitChanges: async () => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    const promises: Promise<any>[] = [];
    const restartKeys: string[] = [];

    for (const domain in draftSettings) {
      const d = domain as keyof VoxSettings;
      for (const key in draftSettings[d]) {
        const val = (draftSettings[d] as any)[key];
        const oldVal = (settings[d] as any)[key];

        if (JSON.stringify(val) !== JSON.stringify(oldVal)) {
          if (domain === "interaction" && (key === "main_app_mode" || key === "tray_mode")) {
            const target = key === "main_app_mode" ? "main" : "tray";
            promises.push(updateInteractionMode(target, val));
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

    await Promise.all(promises);
    set({ hasChanges: false });
    const bootState = await requestBootState();
    const fetched = bootState.settings;
    const cloned = structuredClone(fetched);
    set({ settings: fetched, draftSettings: cloned, hasChanges: false, isLoading: false });

    if (restartKeys.length > 0) {
      set({ restartKeys });
    }
  },

  discardChanges: () => {
    const { settings } = get();
    if (!settings) return;
    const cloned = structuredClone(settings);
    applyAppearance(settings.ui);
    set({ draftSettings: cloned, hasChanges: false });
  },

  restoreDefaults: async () => {
    try {
      const defaults = await resetSettings();
      const cloned = structuredClone(defaults);
      applyAppearance(defaults.ui);
      set({ settings: defaults, draftSettings: cloned, hasChanges: false });
    } catch (err) {
      console.error("Failed to restore defaults:", err);
    }
  },

  toggleTheme: () => {
    const { draftSettings } = get();
    if (!draftSettings) return;
    const newTheme = draftSettings.ui.theme === "dark" ? "light" : "dark";
    get().updateDraft("ui", "theme", newTheme);
  },

  clearRestartKeys: () => {
    set({ restartKeys: [] });
  },
}));
