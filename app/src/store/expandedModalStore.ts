import { create } from "zustand";

interface ExpandedModalState {
  openCount: number;
  pushModal: () => void;
  popModal: () => void;
}

/**
 * Tracks how many ExpandableList modals are open app-wide. Background settings
 * cards suppress their commit footers while any modal owns the commit flow.
 */
export const useExpandedModalStore = create<ExpandedModalState>((set) => ({
  openCount: 0,
  pushModal: () => set((s) => ({ openCount: s.openCount + 1 })),
  popModal: () => set((s) => ({ openCount: Math.max(0, s.openCount - 1) })),
}));
