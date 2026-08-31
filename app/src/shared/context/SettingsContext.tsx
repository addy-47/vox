import React, { createContext, useEffect, useMemo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { onSettingsUpdated } from "@/services/eventsService";

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

export const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export const SettingsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  useEffect(() => {
    useSettingsStore.getState().loadSettings();
    useSettingsStore.getState().loadModelCatalog();
  }, []);

  useEffect(() => {
    const unlisteners: (() => void)[] = [];
    let debounceTimer: ReturnType<typeof setTimeout> | null = null;
    let isMounted = true;

    try {
      unlisteners.push(
        onSettingsUpdated(() => {
          if (!isMounted || useSettingsStore.getState().isCommitting) return;
          if (debounceTimer) clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            if (!isMounted || useSettingsStore.getState().isCommitting) return;
            useSettingsStore.getState().loadSettings();
          }, 80);
        })
      );
    } catch (err) {
      console.warn("[SettingsContext] Failed to bind listeners:", err);
    }

    return () => {
      isMounted = false;
      unlisteners.forEach((u) => u());
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

