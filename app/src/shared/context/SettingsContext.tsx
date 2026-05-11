import React, { createContext, useContext, useEffect, useState, useCallback } from "react";
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
    tray_hide_delay: number;
    tray_fade_transition: string;
    tray_history_limit: number;
  };
  vad: {
    threshold: number;
    ptt_noise_gate: number;
  };
  asr: {
    model: string;
  };
  llm: {
    model: string;
    ctx_size: number;
    threads: number;
  };
  tts: {
    en_model: string;
    hi_model: string;
    voice_id: number;
  };
  interaction: {
    main_app_mode: "Passive" | "PTT";
    tray_mode: "Passive" | "PTT";
  };
  telemetry: {
    enabled: boolean;
    log_level: string;
  };
  persistence: {
    enabled: boolean;
    max_sessions: number;
    retention_days: number;
  };
  assistant: {
    system_prompt: string;
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

  const fetchSettings = async () => {
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
  };

  useEffect(() => {
    fetchSettings();

    let unlisteners: (() => void)[] = [];
    const setup = async () => {
      const win = getCurrentWindow();
      
      const u1 = await win.listen<string>("theme-changed", (event) => {
         const newTheme = event.payload;
         document.documentElement.setAttribute("data-theme", newTheme);
         fetchSettings();
      });

      const u2 = await win.listen("settings-updated", () => {
         fetchSettings();
      });

      unlisteners = [u1, u2];
    };
    setup();
    return () => { unlisteners.forEach(u => u()); };
  }, [applyAppearance]);

  const updateDraft = (domain: keyof VoxSettings, key: string, value: any) => {
    if (!draftSettings) return;

    // Use deep copy to avoid reference sharing with state
    const newDraft = JSON.parse(JSON.stringify(draftSettings));
    (newDraft[domain] as any)[key] = value;
    setDraftSettings(newDraft);

    if (domain === "ui" && (key === "theme" || key === "accent_seed")) {
      applyAppearance(newDraft.ui);
      invoke("update_setting", { domain, key, value });
    }
    
    if (domain === "interaction") {
      const target = key === "main_app_mode" ? "main" : "tray";
      const mode = value;
      invoke("update_interaction_mode", { target, mode });
    }
  };

  const commitChanges = async () => {
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
             if (res.reload_policy === "restart") {
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
  };

  const handleRestart = async () => {
    console.log("Relaunching application...");
    setIsRestartModalOpen(false);
  };

  const discardChanges = () => {
    if (settings) {
      setDraftSettings(JSON.parse(JSON.stringify(settings)));
      applyAppearance(settings.ui);
    }
  };

  const restoreDefaults = async () => {
    try {
      const defaults = await invoke<VoxSettings>("reset_settings");
      setSettings(defaults);
      setDraftSettings(JSON.parse(JSON.stringify(defaults)));
      applyAppearance(defaults.ui);
    } catch (err) {
      console.error("Failed to restore defaults:", err);
    }
  };

  const toggleTheme = () => {
    if (!draftSettings) return;
    const newTheme = draftSettings.ui.theme === "dark" ? "light" : "dark";
    updateDraft("ui", "theme", newTheme);
  };

  const hasChanges = settings && draftSettings ? JSON.stringify(settings) !== JSON.stringify(draftSettings) : false;

  return (
    <SettingsContext.Provider value={{ 
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
    }}>
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
