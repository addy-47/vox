import React, { createContext, useContext, useCallback, useMemo, useState, memo } from "react";

export type PanelId = "help" | "notifications" | "sessions";
export type PanelEdge = "left" | "right";

export const PANEL_EDGE_MAP: Record<PanelId, PanelEdge> = {
  sessions: "left",
  help: "right",
  notifications: "right",
};

interface PanelState {
  activePanel: PanelId | null;
  leftPanel: PanelId | null;
  rightPanel: PanelId | null;
  openPanel: (id: PanelId) => void;
  closePanel: (id: PanelId) => void;
  togglePanel: (id: PanelId) => void;
  closeRightGroup: () => void;
  closeLeftGroup: () => void;
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
  const [leftPanel, setLeftPanel] = useState<PanelId | null>(null);
  const [rightPanel, setRightPanel] = useState<PanelId | null>(null);

  const openPanel = useCallback((id: PanelId) => {
    const edge = PANEL_EDGE_MAP[id];
    if (edge === "left") {
      setLeftPanel(id);
    } else {
      setRightPanel(id);
    }
  }, []);

  const closePanel = useCallback((id: PanelId) => {
    const edge = PANEL_EDGE_MAP[id];
    if (edge === "left") {
      setLeftPanel((prev) => (prev === id ? null : prev));
    } else {
      setRightPanel((prev) => (prev === id ? null : prev));
    }
  }, []);

  const togglePanel = useCallback((id: PanelId) => {
    const edge = PANEL_EDGE_MAP[id];
    if (edge === "left") {
      setLeftPanel((prev) => (prev === id ? null : id));
    } else {
      setRightPanel((prev) => (prev === id ? null : id));
    }
  }, []);

  const closeRightGroup = useCallback(() => {
    setRightPanel(null);
  }, []);

  const closeLeftGroup = useCallback(() => {
    setLeftPanel(null);
  }, []);

  const isPanelOpen = useCallback(
    (id: PanelId) => {
      const edge = PANEL_EDGE_MAP[id];
      return edge === "left" ? leftPanel === id : rightPanel === id;
    },
    [leftPanel, rightPanel]
  );

  const activePanel = useMemo<PanelId | null>(() => {
    return rightPanel ?? leftPanel;
  }, [rightPanel, leftPanel]);

  const value = useMemo<PanelState>(
    () => ({
      activePanel,
      leftPanel,
      rightPanel,
      openPanel,
      closePanel,
      togglePanel,
      closeRightGroup,
      closeLeftGroup,
      isPanelOpen,
    }),
    [
      activePanel,
      leftPanel,
      rightPanel,
      openPanel,
      closePanel,
      togglePanel,
      closeRightGroup,
      closeLeftGroup,
      isPanelOpen,
    ]
  );

  return (
    <PanelStateContext.Provider value={value}>
      {children}
    </PanelStateContext.Provider>
  );
});
PanelStateProvider.displayName = "PanelStateProvider";
