import { create } from "zustand";

export type SessionPanelView = "projects" | "history";

interface SessionPanelUiState {
  viewMode: SessionPanelView;
  expandedProjectIds: string[];
  setViewMode: (view: SessionPanelView) => void;
  toggleProjectExpanded: (projectId: string) => void;
}

/**
 * UI state for the sessions panel. The panel unmounts on close (EdgePanel +
 * AnimatePresence), so this lives outside the component tree to survive
 * open/close cycles.
 */
export const useSessionPanelUiStore = create<SessionPanelUiState>((set) => ({
  viewMode: "projects",
  expandedProjectIds: [],
  setViewMode: (viewMode) => set({ viewMode }),
  toggleProjectExpanded: (projectId) =>
    set((s) => ({
      expandedProjectIds: s.expandedProjectIds.includes(projectId)
        ? s.expandedProjectIds.filter((id) => id !== projectId)
        : [...s.expandedProjectIds, projectId],
    })),
}));

/**
 * Scroll offset of the panel's scroll container. Plain mutable box (not
 * store state) so writing it never re-renders subscribers.
 */
export const sessionPanelScrollTop = { value: 0 };
