import { create } from "zustand";
import type { MemoryComment } from "@/shared/components/memory/PersonalMemoryStagingCard";

interface MemoryStoreState {
  /** Pending inline comments that survive navigation */
  pendingComments: MemoryComment[];
  /** Whether to auto-open the comment queue section when the drawer next opens */
  reopenToComments: boolean;

  // Actions
  setComments: (comments: MemoryComment[]) => void;
  addComment: (comment: MemoryComment) => void;
  updateComment: (id: string, text: string) => void;
  deleteComment: (id: string) => void;
  clearComments: () => void;
  setReopenToComments: (value: boolean) => void;
}

export const useMemoryStore = create<MemoryStoreState>((set) => ({
  pendingComments: [],
  reopenToComments: false,

  setComments: (comments) => set({ pendingComments: comments }),
  addComment: (comment) =>
    set((state) => ({ pendingComments: [...state.pendingComments, comment] })),
  updateComment: (id, text) =>
    set((state) => ({
      pendingComments: state.pendingComments.map((c) =>
        c.id === id ? { ...c, text } : c
      ),
    })),
  deleteComment: (id) =>
    set((state) => ({
      pendingComments: state.pendingComments.filter((c) => c.id !== id),
    })),
  clearComments: () => set({ pendingComments: [], reopenToComments: false }),
  setReopenToComments: (value) => set({ reopenToComments: value }),
}));
