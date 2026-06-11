import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { hexToRgb } from "@/shared/lib/utils";

export interface ModelMetadata {
  id: string;
  name: string;
  description: string;
  ram_usage: string;
  parameters: string;
  tradeoffs?: string;
}

export interface VoiceProfile {
  id: number;
  name: string;
}

export interface ModelCatalog {
  llm: ModelMetadata[];
  asr: ModelMetadata[];
  tts: ModelMetadata[];
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
  };
  llm: {
    model: string;
    ctx_size: number;
    threads: number;
  };
  tts: {
    voice: number;
    quality_steps: number;
    speed: number;
  };
  interaction: {
    main_app_mode: "Passive" | "PTT";
    tray_mode: "Passive" | "PTT";
    auto_sleep_timeout: number;
  };
  telemetry: {
    enabled: boolean;
    log_level: string;
  };
  persistence: {
    private_mode: boolean;
    max_sessions: number;
    retention_days: number;
  };
  assistant: {
    hindi_prompt: string;
    english_prompt: string;
  };
  setup: {
    completed: boolean;
  };
}

interface SettingsState {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  modelCatalog: ModelCatalog | null;
  isLoading: boolean;
  hasChanges: boolean;
  restartKeys: string[];

  loadSettings: () => Promise<void>;
  loadModelCatalog: () => Promise<void>;
  updateDraft: (domain: keyof VoxSettings, key: string, value: any) => void;
  commitChanges: () => Promise<void>;
  discardChanges: () => void;
  restoreDefaults: () => Promise<void>;
  toggleTheme: () => void;
  clearRestartKeys: () => void;
}

function applyAppearance(ui: VoxSettings["ui"]) {
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", ui.theme);
    document.documentElement.style.setProperty("--accent", hexToRgb(ui.accent_seed));
  }
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  draftSettings: null,
  modelCatalog: null,
  isLoading: true,
  hasChanges: false,
  restartKeys: [],

  loadSettings: async () => {
    try {
      const bootState = await invoke<{ settings: VoxSettings }>("request_boot_state");
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);
      set({
        settings: fetched,
        draftSettings: cloned,
        isLoading: false,
        hasChanges: false,
      });
      applyAppearance(fetched.ui);
    } catch (err) {
      console.error("Failed to load settings:", err);
      set({ isLoading: false });
    }
  },

  loadModelCatalog: async () => {
    try {
      const catalog = await invoke<ModelCatalog>("request_model_catalog");
      set({ modelCatalog: catalog });
    } catch (err) {
      console.error("Failed to load model catalog:", err);
    }
  },

  updateDraft: (domain, key, value) => {
    const { settings, draftSettings } = get();
    if (!draftSettings || !settings) return;

    const newDraft = structuredClone(draftSettings);
    (newDraft[domain] as any)[key] = value;

    if (domain === "ui" && (key === "theme" || key === "accent_seed")) {
      applyAppearance(newDraft.ui);
      invoke("update_setting", { domain, key, value }).catch(console.error);
      
      // Update baseline settings immediately so appearance has no unsaved changes state
      const newSettings = structuredClone(settings);
      (newSettings.ui as any)[key] = value;
      set({ settings: newSettings, draftSettings: newDraft, hasChanges: false });
      return;
    }

    const hasChanges = JSON.stringify(settings) !== JSON.stringify(newDraft);
    set({ draftSettings: newDraft, hasChanges });
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
            promises.push(invoke("update_interaction_mode", { target, mode: val }));
          } else {
            promises.push(
              invoke("update_setting", { domain, key, value: val }).then((res: any) => {
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
    await get().loadSettings();

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
      const defaults = await invoke<VoxSettings>("reset_settings");
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
