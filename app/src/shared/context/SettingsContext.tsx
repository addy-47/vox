import React, { createContext, useContext, useEffect, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { hexToRgb } from "@/shared/lib/utils";
import { RestartModal } from "@/shared/components/RestartModal";

export interface ModelMetadata {
  id: string;
  name: string;
  description: string;
  ram_usage: string;
  parameters: string;
}

export interface VoiceProfile {
  id: number;
  name: string;
  language: string;
  model_file?: string;
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
    en_model: string;
    en_voice: number;
    hi_model: string;
    hi_voice: string;
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
    enabled: boolean;
    private_mode: boolean;
    max_sessions: number;
    retention_days: number;
  };
  assistant: {
    system_prompt: string;
    hindi_prompt: string;
    english_prompt: string;
  };
}

interface SettingsContextType {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  isLoading: boolean;
  hasChanges: boolean;
  updateDraft: (domain: keyof VoxSettings, key: string, value: any) => void;
  commitChanges: () => Promise<void>;
  discardChanges: () => void;
  restoreDefaults: () => Promise<void>;
  toggleTheme: () => void;
  modelCatalog: ModelCatalog | null;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export const SettingsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [settings, setSettings] = useState<VoxSettings | null>(null);
  const [draftSettings, setDraftSettings] = useState<VoxSettings | null>(null);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalog | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRestartModalOpen, setIsRestartModalOpen] = useState(false);
  const [changedRestartKeys, setChangedRestartKeys] = useState<string[]>([]);

  const applyAppearance = useCallback((ui: VoxSettings["ui"]) => {
    document.documentElement.setAttribute("data-theme", ui.theme);
    document.documentElement.style.setProperty("--accent", hexToRgb(ui.accent_seed));
  }, []);

  const fetchSettings = useCallback(async () => {
    try {
      const [settingsResp, catalogResp] = await Promise.all([
        invoke<VoxSettings>("get_settings"),
        invoke<ModelCatalog>("request_model_catalog")
      ]);
      setSettings(settingsResp);
      setDraftSettings(JSON.parse(JSON.stringify(settingsResp)));
      setModelCatalog(catalogResp);
      applyAppearance(settingsResp.ui);
    } catch (err) {
      console.error("Failed to fetch settings:", err);
    } finally {
      setIsLoading(false);
    }
  }, [applyAppearance]);

  useEffect(() => {
    fetchSettings();

    let unlisteners: (() => void)[] = [];
    const setup = async () => {
      const win = getCurrentWindow();
      
      // Only fetch if another window updated settings
      // We skip local updates because updateDraft already handles local state
      const u1 = await win.listen<string>("theme-changed", (_) => {
         // Optionally check if event.windowLabel !== currentWindow
         fetchSettings();
      });

      const u2 = await win.listen("settings-updated", () => {
         // This is still needed for sync across windows, 
         // but we can make it less aggressive
         fetchSettings();
      });

      unlisteners = [u1, u2];
    };
    setup();
    return () => { unlisteners.forEach(u => u()); };
  }, [fetchSettings]);

  const updateDraft = useCallback((domain: keyof VoxSettings, key: string, value: any) => {
    if (!draftSettings) return;

    setDraftSettings(prev => {
      if (!prev) return prev;
      return {
        ...prev,
        [domain]: {
          ...(prev[domain] as any),
          [key]: value
        }
      };
    });

    if (domain === "ui" && (key === "theme" || key === "accent_seed")) {
      // Use the new value directly instead of stale draftSettings
      const updatedUi = { ...draftSettings.ui, [key]: value };
      applyAppearance(updatedUi);
      invoke("update_setting", { domain, key, value });
    }

    // Phase 6: Hot-update private mode in backend immediately
    if (domain === "persistence" && key === "private_mode") {
      invoke("update_setting", { domain, key, value });
    }
    
    if (domain === "interaction") {
      const target = key === "main_app_mode" ? "main" : "tray";
      invoke("update_interaction_mode", { target, mode: value });
    }
  }, [draftSettings, applyAppearance]);

  const commitChanges = useCallback(async () => {
    if (!settings || !draftSettings) return;

    const promises: Promise<any>[] = [];
    const restartKeys: string[] = [];

    for (const domain in draftSettings) {
      const d = domain as keyof VoxSettings;
      for (const key in draftSettings[d]) {
        const val = (draftSettings[d] as any)[key];
        const oldVal = (settings[d] as any)[key];

        if (JSON.stringify(val) !== JSON.stringify(oldVal)) {
          promises.push(invoke("update_setting", { domain, key, value: val }).then((res: any) => {
             if (res && res.reload_policy === "restart") {
                restartKeys.push(`${domain}.${key}`);
             }
          }));
        }
      }
    }

    await Promise.all(promises);
    await fetchSettings();

    if (restartKeys.length > 0) {
      setChangedRestartKeys(restartKeys);
      setIsRestartModalOpen(true);
    }
  }, [settings, draftSettings, fetchSettings]);

  const handleRestart = useCallback(async () => {
    setIsRestartModalOpen(false);
    // Add tauri relaunch logic if available
  }, []);

  const discardChanges = useCallback(() => {
    if (settings) {
      setDraftSettings(JSON.parse(JSON.stringify(settings)));
      applyAppearance(settings.ui);
    }
  }, [settings, applyAppearance]);

  const restoreDefaults = useCallback(async () => {
    try {
      const defaults = await invoke<VoxSettings>("reset_settings");
      setSettings(defaults);
      setDraftSettings(JSON.parse(JSON.stringify(defaults)));
      applyAppearance(defaults.ui);
    } catch (err) {
      console.error("Failed to restore defaults:", err);
    }
  }, [applyAppearance]);

  const toggleTheme = useCallback(() => {
    if (!draftSettings) return;
    const newTheme = draftSettings.ui.theme === "dark" ? "light" : "dark";
    updateDraft("ui", "theme", newTheme);
  }, [draftSettings, updateDraft]);

  const hasChanges = useMemo(() => {
    return settings && draftSettings ? JSON.stringify(settings) !== JSON.stringify(draftSettings) : false;
  }, [settings, draftSettings]);

  const value = useMemo(() => ({
    settings,
    draftSettings,
    isLoading,
    hasChanges,
    updateDraft,
    commitChanges,
    discardChanges,
    restoreDefaults,
    toggleTheme,
    modelCatalog
  }), [settings, draftSettings, isLoading, hasChanges, updateDraft, commitChanges, discardChanges, restoreDefaults, toggleTheme, modelCatalog]);

  return (
    <SettingsContext.Provider value={value}>
      {children}
      <RestartModal 
        isOpen={isRestartModalOpen}
        onClose={() => setIsRestartModalOpen(false)}
        onRestart={handleRestart}
        changedSettings={changedRestartKeys}
      />
    </SettingsContext.Provider>
  );
};

export const useSettings = () => {
  const context = useContext(SettingsContext);
  if (!context) throw new Error("useSettings must be used within SettingsProvider");
  return context;
};
