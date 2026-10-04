import { create } from "zustand";
import type { SessionRow } from "@/services/historyService";

export type HistoryDateFilter = "all" | "today" | "week" | "month" | "custom";
export type HistoryDisplayMode = "orbit" | "list";

interface HistoryFilterState {
  dateFilter: HistoryDateFilter;
  displayMode: HistoryDisplayMode;
  drillDownSession: SessionRow | null;
  setDateFilter: (filter: HistoryDateFilter) => void;
  setDisplayMode: (mode: HistoryDisplayMode) => void;
  setDrillDownSession: (session: SessionRow | null) => void;
}

export const useHistoryFilterStore = create<HistoryFilterState>((set) => ({
  dateFilter: "all",
  displayMode: "orbit",
  drillDownSession: null,
  setDateFilter: (filter: HistoryDateFilter) => set({ dateFilter: filter }),
  setDisplayMode: (mode: HistoryDisplayMode) => set({ displayMode: mode }),
  setDrillDownSession: (session: SessionRow | null) => set({ drillDownSession: session }),
}));

