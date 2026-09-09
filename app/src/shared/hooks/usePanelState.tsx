import React, { createContext, useContext, useCallback, useMemo, useState, memo } from "react";

export type PanelId = "help" | "notifications" | "sessions";

interface PanelState {
  activePanel: PanelId | null;
  openPanel: (id: PanelId) => void;
  closePanel: (id: PanelId) => void;
  togglePanel: (id: PanelId) => void;
  closeRightGroup: () => void;
  isPanelOpen: (id: PanelId) => boolean;
}

const PanelStateContext = createContext<PanelState | null>(null);

export function usePanelStateContext(): PanelState {
  const ctx = useContext(PanelStateContext);
  if (!ctx) throw new Error("usePanelState must be used within PanelStateProvider");
  return ctx;
}

interface PanelStateProviderProps {
  children: React.ReactNode;
}

export const PanelStateProvider: React.FC<PanelStateProviderProps> = memo(({ children }) => {
  const [activePanel, setActivePanel] = useState<PanelId | null>(null);

  const openPanel = useCallback((id: PanelId) => {
    setActivePanel((prev) => {
      if (prev === id) return null;
      if ((id === "help" || id === "notifications") && (prev === "help" || prev === "notifications")) {
        return id;
      }
      return id;
    });
  }, []);

  const closePanel = useCallback((id: PanelId) => {
    setActivePanel((prev) => (prev === id ? null : prev));
  }, []);

  const togglePanel = useCallback((id: PanelId) => {
    setActivePanel((prev) => {
      if (prev === id) return null;
      return id;
    });
  }, []);

  const closeRightGroup = useCallback(() => {
    setActivePanel((prev) =>
      prev === "help" || prev === "notifications" ? null : prev
    );
  }, []);

  const isPanelOpen = useCallback(
    (id: PanelId) => activePanel === id,
    [activePanel]
  );

  const value = useMemo<PanelState>(
    () => ({ activePanel, openPanel, closePanel, togglePanel, closeRightGroup, isPanelOpen }),
    [activePanel, openPanel, closePanel, togglePanel, closeRightGroup, isPanelOpen]
  );

  return (
    <PanelStateContext.Provider value={value}>
      {children}
    </PanelStateContext.Provider>
  );
});
PanelStateProvider.displayName = "PanelStateProvider";
