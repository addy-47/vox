import { create } from "zustand";
import type { InteractionState, TurnMetricsPayload, ActivityEnvelope } from "@/services/eventsService";
import { compactSession } from "@/services/historyService";
import type { InteractionModeUpper } from "@/shared/lib/interactionMode";

export interface DialogueTurn {
  user: string;
  assistant: string;
  id: number;
}

export interface SessionStoreState {
  // Core Pipeline & Mode State
  interactionState: InteractionState;
  interactionMode: InteractionModeUpper;
  pipelineMode: "modular" | "realtime";
  isLaunching: boolean;
  activeActivity: ActivityEnvelope | null;

  // Streaming Text & Diagnostics
  transcript: string;
  assistantText: string;
  sessionError: string | null;
  cpuWarning: { governor: string } | null;

  // Text Mode, Temporary Session & Mute Controls
  isTemporarySession: boolean;
  isTextModeOpen: boolean;
  isPlaybackMuted: boolean;
  isMicMuted: boolean;

  // Session & Turn History
  activeSessionId: number | null;
  dialogueHistory: DialogueTurn[];
  turnIdCounter: number;
  latestTurnMetrics: TurnMetricsPayload | null;

  // Session Continuation / Restore State
  isRestoring: boolean;
  restoringSessionId: number | null;
  restoreError: string | null;
  restoreSignal: number;
  sessionListVersion: number;

  // Compaction State
  compactingSessionId: number | null;

  // Actions
  setInteractionState: (state: InteractionState) => void;
  setInteractionMode: (mode: InteractionModeUpper) => void;
  setPipelineMode: (mode: "modular" | "realtime") => void;
  setIsLaunching: (launching: boolean) => void;
  setTranscript: (text: string) => void;
  setAssistantText: (text: string) => void;
  setSessionError: (error: string | null) => void;
  setCpuWarning: (warning: { governor: string } | null) => void;
  setIsTemporarySession: (isTemporarySession: boolean) => void;
  setIsTextModeOpen: (isTextModeOpen: boolean) => void;
  setIsPlaybackMuted: (isPlaybackMuted: boolean) => void;
  setIsMicMuted: (isMicMuted: boolean) => void;
  setActiveActivity: (activity: ActivityEnvelope | null) => void;
  setActiveSessionId: (id: number | null) => void;
  setDialogueHistory: (history: DialogueTurn[] | ((prev: DialogueTurn[]) => DialogueTurn[])) => void;
  setTurnIdCounter: (counter: number | ((prev: number) => number)) => void;
  setLatestTurnMetrics: (metrics: TurnMetricsPayload | null) => void;
  setIsRestoring: (restoring: boolean) => void;
  setRestoringSessionId: (id: number | null) => void;
  setRestoreError: (error: string | null) => void;
  setRestoreSignal: (signal: number) => void;
  bumpSessionListVersion: () => void;
  setCompactingSessionId: (id: number | null) => void;
  executeCompaction: (sessionId: number) => Promise<void>;
  resetSessionState: () => void;
  /** Breadcrumb shown in layout when panel is closed: { sessionTitle, projectName } */
  activeSessionLabel: { sessionTitle: string | null; projectName: string | null };
  setActiveSessionLabel: (label: { sessionTitle: string | null; projectName: string | null }) => void;
}

const INITIAL_STATE = {
  interactionState: "Idle" as InteractionState,
  interactionMode: "PASSIVE" as InteractionModeUpper,
  pipelineMode: "modular" as const,
  isLaunching: false,
  activeActivity: null as ActivityEnvelope | null,
  transcript: "",
  assistantText: "",
  sessionError: null,
  cpuWarning: null,
  isTemporarySession: false,
  isTextModeOpen: false,
  isPlaybackMuted: false,
  isMicMuted: false,
  activeSessionId: null,
  dialogueHistory: [],
  turnIdCounter: 0,
  latestTurnMetrics: null as TurnMetricsPayload | null,
  isRestoring: false,
  restoringSessionId: null,
  restoreError: null,
  restoreSignal: 0,
  sessionListVersion: 0,
  compactingSessionId: null as number | null,
  activeSessionLabel: { sessionTitle: null, projectName: null },
};

export const useSessionStore = create<SessionStoreState>((set) => ({
  ...INITIAL_STATE,

  setInteractionState: (interactionState) => set({ interactionState }),
  setActiveActivity: (activeActivity) => set({ activeActivity }),
  setInteractionMode: (interactionMode) => set({ interactionMode }),
  setPipelineMode: (pipelineMode) => set({ pipelineMode }),
  setIsLaunching: (isLaunching) => set({ isLaunching }),
  setTranscript: (transcript) => set({ transcript }),
  setAssistantText: (assistantText) => set({ assistantText }),
  setSessionError: (sessionError) => set({ sessionError }),
  setCpuWarning: (cpuWarning) => set({ cpuWarning }),
  setIsTemporarySession: (isTemporarySession) => set({ isTemporarySession }),
  setIsTextModeOpen: (isTextModeOpen) => set({ isTextModeOpen }),
  setIsPlaybackMuted: (isPlaybackMuted) => set({ isPlaybackMuted }),
  setIsMicMuted: (isMicMuted) => set({ isMicMuted }),
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),
  setDialogueHistory: (historyOrUpdater) =>
    set((state) => ({
      dialogueHistory:
        typeof historyOrUpdater === "function"
          ? historyOrUpdater(state.dialogueHistory)
          : historyOrUpdater,
    })),
  setTurnIdCounter: (counterOrUpdater) =>
    set((state) => ({
      turnIdCounter:
        typeof counterOrUpdater === "function"
          ? counterOrUpdater(state.turnIdCounter)
          : counterOrUpdater,
    })),
  setLatestTurnMetrics: (latestTurnMetrics) => set({ latestTurnMetrics }),
  setIsRestoring: (isRestoring) => set({ isRestoring }),
  setRestoringSessionId: (restoringSessionId) => set({ restoringSessionId }),
  setRestoreError: (restoreError) => set({ restoreError }),
  setRestoreSignal: (restoreSignal) => set({ restoreSignal }),
  bumpSessionListVersion: () => set((state) => ({ sessionListVersion: state.sessionListVersion + 1 })),
  setActiveSessionLabel: (activeSessionLabel) => set({ activeSessionLabel }),
  setCompactingSessionId: (compactingSessionId) => set({ compactingSessionId }),
  executeCompaction: async (sessionId: number) => {
    if (useSessionStore.getState().compactingSessionId !== null) return;
    set({ compactingSessionId: sessionId });
    try {
      const res = await compactSession(sessionId);
      if (res && res.status === "failed") {
        console.warn("[SessionStore] Compaction returned failure:", res.error);
      }
    } catch (e) {
      console.error("[SessionStore] Failed to trigger compaction:", e);
    } finally {
      set({ compactingSessionId: null });
    }
  },
  resetSessionState: () =>
    set({
      interactionState: "Idle",
      transcript: "",
      assistantText: "",
      sessionError: null,
      activeSessionId: null,
      dialogueHistory: [],
      turnIdCounter: 0,
      latestTurnMetrics: null,
    }),
}));
