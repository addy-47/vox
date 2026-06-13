import React, { createContext, useContext, useEffect, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsStore } from "@/store/settingsStore";

export type { VoxSettings, ModelMetadata, VoiceProfile, ModelCatalog } from "@/store/settingsStore";
import type { VoxSettings, ModelCatalog } from "@/store/settingsStore";

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
  useEffect(() => {
    useSettingsStore.getState().loadSettings();
    useSettingsStore.getState().loadModelCatalog();
  }, []);

  useEffect(() => {
    let unlisteners: (() => void)[] = [];
    let debounceTimer: any = null;

    const setup = async () => {
      const win = getCurrentWindow();
      const u1 = await win.listen<string>("theme-changed", () => {
        useSettingsStore.getState().loadSettings();
      });
      const u2 = await win.listen("settings-updated", () => {
        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
          useSettingsStore.getState().loadSettings();
        }, 80);
      });
      unlisteners = [u1, u2];
    };
    setup();
    return () => { 
      unlisteners.forEach(u => u()); 
      if (debounceTimer) clearTimeout(debounceTimer);
    };
  }, []);

  const settings = useSettingsStore(s => s.settings);
  const draftSettings = useSettingsStore(s => s.draftSettings);
  const modelCatalog = useSettingsStore(s => s.modelCatalog);
  const isLoading = useSettingsStore(s => s.isLoading);
  const hasChanges = useSettingsStore(s => s.hasChanges);
  const updateDraft = useSettingsStore(s => s.updateDraft);
  const commitChanges = useSettingsStore(s => s.commitChanges);
  const discardChanges = useSettingsStore(s => s.discardChanges);
  const restoreDefaults = useSettingsStore(s => s.restoreDefaults);
  const toggleTheme = useSettingsStore(s => s.toggleTheme);

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
    modelCatalog,
  }), [settings, draftSettings, isLoading, hasChanges, updateDraft, commitChanges, discardChanges, restoreDefaults, toggleTheme, modelCatalog]);

  return (
    <SettingsContext.Provider value={value}>
      {children}
    </SettingsContext.Provider>
  );
};

export const useSettings = () => {
  const context = useContext(SettingsContext);
  if (!context) throw new Error("useSettings must be used within SettingsProvider");
  return context;
};
