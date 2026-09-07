import { create } from "zustand";
import type { InteractionState } from "@/services/eventsService";
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

  // Streaming Text & Diagnostics
  transcript: string;
  assistantText: string;
  sessionError: string | null;
  cpuWarning: { governor: string } | null;

  // Test Clip Support
  testMode: boolean;
  testingClip: string | null;

  // Session & Turn History
  activeSessionId: number | null;
  dialogueHistory: DialogueTurn[];
  turnIdCounter: number;

  // Session Continuation / Restore State
  isRestoring: boolean;
  restoreError: string | null;
  restoreSignal: number;
  sessionListVersion: number;

  // Actions
  setInteractionState: (state: InteractionState) => void;
  setInteractionMode: (mode: InteractionModeUpper) => void;
  setPipelineMode: (mode: "modular" | "realtime") => void;
  setIsLaunching: (launching: boolean) => void;
  setTranscript: (text: string) => void;
  setAssistantText: (text: string) => void;
  setSessionError: (error: string | null) => void;
  setCpuWarning: (warning: { governor: string } | null) => void;
  setTestMode: (testMode: boolean) => void;
  setTestingClip: (clipId: string | null) => void;
  setActiveSessionId: (id: number | null) => void;
  setDialogueHistory: (history: DialogueTurn[] | ((prev: DialogueTurn[]) => DialogueTurn[])) => void;
  setTurnIdCounter: (counter: number | ((prev: number) => number)) => void;
  setIsRestoring: (restoring: boolean) => void;
  setRestoreError: (error: string | null) => void;
  setRestoreSignal: (signal: number) => void;
  bumpSessionListVersion: () => void;
  resetSessionState: () => void;
}

const INITIAL_STATE = {
  interactionState: "Idle" as InteractionState,
  interactionMode: "PASSIVE" as InteractionModeUpper,
  pipelineMode: "modular" as const,
  isLaunching: false,
  transcript: "",
  assistantText: "",
  sessionError: null,
  cpuWarning: null,
  testMode: false,
  testingClip: null,
  activeSessionId: null,
  dialogueHistory: [],
  turnIdCounter: 0,
  isRestoring: false,
  restoreError: null,
  restoreSignal: 0,
  sessionListVersion: 0,
};

export const useSessionStore = create<SessionStoreState>((set) => ({
  ...INITIAL_STATE,

  setInteractionState: (interactionState) => set({ interactionState }),
  setInteractionMode: (interactionMode) => set({ interactionMode }),
  setPipelineMode: (pipelineMode) => set({ pipelineMode }),
  setIsLaunching: (isLaunching) => set({ isLaunching }),
  setTranscript: (transcript) => set({ transcript }),
  setAssistantText: (assistantText) => set({ assistantText }),
  setSessionError: (sessionError) => set({ sessionError }),
  setCpuWarning: (cpuWarning) => set({ cpuWarning }),
  setTestMode: (testMode) => set({ testMode }),
  setTestingClip: (testingClip) => set({ testingClip }),
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
  setIsRestoring: (isRestoring) => set({ isRestoring }),
  setRestoreError: (restoreError) => set({ restoreError }),
  setRestoreSignal: (restoreSignal) => set({ restoreSignal }),
  bumpSessionListVersion: () => set((state) => ({ sessionListVersion: state.sessionListVersion + 1 })),
  resetSessionState: () =>
    set({
      interactionState: "Idle",
      transcript: "",
      assistantText: "",
      sessionError: null,
      activeSessionId: null,
      dialogueHistory: [],
      turnIdCounter: 0,
    }),
}));
