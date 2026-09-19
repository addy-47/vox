import { create } from "zustand";

export type HistoryDateFilter = "all" | "today" | "week" | "month" | "custom";
export type HistoryDisplayMode = "orbit" | "list";

interface HistoryFilterState {
  dateFilter: HistoryDateFilter;
  displayMode: HistoryDisplayMode;
  setDateFilter: (filter: HistoryDateFilter) => void;
  setDisplayMode: (mode: HistoryDisplayMode) => void;
}

export const useHistoryFilterStore = create<HistoryFilterState>((set) => ({
  dateFilter: "all",
  displayMode: "orbit",
  setDateFilter: (filter: HistoryDateFilter) => set({ dateFilter: filter }),
  setDisplayMode: (mode: HistoryDisplayMode) => set({ displayMode: mode }),
}));

