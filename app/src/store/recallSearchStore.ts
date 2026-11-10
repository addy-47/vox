import { create } from "zustand";

export type SortOrder = "newest" | "oldest" | "turns";

interface RecallSearchState {
  /** Whether the user is currently in Search / Recall Mode on Home */
  isSearchMode: boolean;
  /** Active search query string */
  searchQuery: string;
  /** Current sorting order */
  sortOrder: SortOrder;
  /** User-navigated card index (overrides default newest/oldest focus) */
  userNavigatedIndex: number | null;

  setIsSearchMode: (active: boolean) => void;
  setSearchQuery: (query: string) => void;
  setSortOrder: (order: SortOrder) => void;
  setUserNavigatedIndex: (index: number | null | ((prev: number | null) => number | null)) => void;
  exitSearchMode: () => void;
}

/**
 * Persistent UI state for Search / Recall Mode.
 *
 * Invariant: Search Mode and its query / carousel state must NOT reset when
 * navigating across pages (e.g. visiting /memory or /settings and returning to
 * /). It exits ONLY when the user explicitly triggers exit (Escape key or
 * "Back to Voice Mode" button).
 */
export const useRecallSearchStore = create<RecallSearchState>((set) => ({
  isSearchMode: false,
  searchQuery: "",
  sortOrder: "newest",
  userNavigatedIndex: null,

  setIsSearchMode: (isSearchMode) => set({ isSearchMode }),
  setSearchQuery: (searchQuery) => set({ searchQuery, userNavigatedIndex: null }),
  setSortOrder: (sortOrder) => set({ sortOrder, userNavigatedIndex: null }),
  setUserNavigatedIndex: (index) =>
    set((s) => ({
      userNavigatedIndex: typeof index === "function" ? index(s.userNavigatedIndex) : index,
    })),
  exitSearchMode: () =>
    set({
      isSearchMode: false,
      searchQuery: "",
      userNavigatedIndex: null,
    }),
}));
