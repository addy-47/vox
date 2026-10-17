import React, { createContext, useContext, useCallback, useMemo, useState, useEffect, useRef, memo } from "react";
import { BREAKPOINT_DUAL_PANEL_MIN } from "@/layout/breakpoints";

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

  // Ref mirror of leftPanel so the resize handler can read it without a
  // setState updater function (updaters must stay pure — no setTimeout inside).
  const leftPanelRef = useRef<PanelId | null>(null);
  useEffect(() => {
    leftPanelRef.current = leftPanel;
  }, [leftPanel]);

  /**
   * Panel Exclusivity Threshold (SSOT: BREAKPOINT_DUAL_PANEL_MIN):
   * On viewports < 1280px (mobile, tablet, and compact/standard desktop windows),
   * only ONE panel edge may be open across the entire page. Opening or toggling
   * any panel closes the opposite edge.
   * On extra-wide viewports (≥ 1280px), left and right rails may coexist.
   */
  const THRESHOLD_DUAL_PANEL_WIDTH = BREAKPOINT_DUAL_PANEL_MIN;

  const isNarrowViewport = useCallback(() => {
    return typeof window !== "undefined" && window.innerWidth < THRESHOLD_DUAL_PANEL_WIDTH;
  }, []);

  // Enforce exclusivity on resize
  useEffect(() => {
    let rAfId: number | null = null;
    const handleResize = () => {
      if (rAfId !== null) return;
      rAfId = requestAnimationFrame(() => {
        rAfId = null;
        if (typeof window !== "undefined" && window.innerWidth < THRESHOLD_DUAL_PANEL_WIDTH) {
          // Read via ref — no updater, no timer, still a single rAF.
          // (Previously a setTimeout() inside a setState updater, which violates
          // the updater-purity rule and left an untracked macrotask per frame.)
          if (leftPanelRef.current !== null) setRightPanel(null);
        }
      });
    };
    window.addEventListener("resize", handleResize, { passive: true });
    return () => {
      window.removeEventListener("resize", handleResize);
      if (rAfId !== null) cancelAnimationFrame(rAfId);
    };
  }, []);

  const openPanel = useCallback((id: PanelId) => {
    const edge = PANEL_EDGE_MAP[id];
    if (edge === "left") {
      setLeftPanel(id);
      if (isNarrowViewport()) setRightPanel(null);
    } else {
      setRightPanel(id);
      if (isNarrowViewport()) setLeftPanel(null);
    }
  }, [isNarrowViewport]);

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
    const isNarrow = isNarrowViewport();
    if (edge === "left") {
      setLeftPanel((prev) => (prev === id ? null : id));
      if (isNarrow) setRightPanel(null);
    } else {
      setRightPanel((prev) => (prev === id ? null : id));
      if (isNarrow) setLeftPanel(null);
    }
  }, [isNarrowViewport]);

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
