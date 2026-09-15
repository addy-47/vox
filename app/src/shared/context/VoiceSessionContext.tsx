import { createContext, useContext, useCallback, useMemo } from "react";
import type { ReactNode } from "react";
import { useSessionStore } from "@/store/sessionStore";
import { useSessionEvents } from "@/shared/hooks/useSessionEvents";
import { useTranscriptStream } from "@/shared/hooks/useTranscriptStream";
import { useSessionHydration } from "@/shared/hooks/useSessionHydration";
import {
  engageSession,
  disengageSession,
  pauseSession,
  resumeSession,
  pttStart,
  pttStop,
  pttCancel,
} from "@/services/sessionService";
import { testClip } from "@/services/pipelineService";
import {
  continueSession as continueSessionIpc,
  createSession as createSessionIpc,
} from "@/services/historyService";
import { getSettings } from "@/services/settingsService";
import { SESSION_COPY } from "@/data/sessionCopy";
import type { InteractionState, StateChangedPayload } from "@/services/eventsService";
import { type InteractionModeUpper, normalizeToInteractionModeUpper } from "@/shared/lib/interactionMode";

export type InteractionMode = InteractionModeUpper;
export type DialogueTurn = { user: string; assistant: string; id: number };

export interface VoiceSessionContextValue {
  interactionState: InteractionState;
  interactionMode: InteractionMode;
  pipelineMode: "modular" | "realtime";
  isEngaged: boolean;
  isSleeping: boolean;
  isPaused: boolean;
  isThinking: boolean;
  isLaunching: boolean;
  transcript: string;
  assistantText: string;
  cpuWarning: { governor: string } | null;
  testMode: boolean;
  testingClip: string | null;
  dialogueHistory: DialogueTurn[];
  activeSessionId: number | null;
  isRestoring: boolean;
  restoringSessionId: number | null;
  restoreError: string | null;
  restoreSignal: number;
  sessionListVersion: number;
  hasCachedSession: boolean;
  pttStatus: "IDLE" | "RECORDING" | "PROCESSING";
  engage: () => Promise<void>;
  disengage: () => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
  handlePttStart: () => Promise<void>;
  handlePttStop: () => Promise<void>;
  handlePttCancel: () => Promise<void>;
  togglePtt: () => Promise<void>;
  handleTestClip: (clipId: string) => Promise<void>;
  selectSession: (sessionId: number) => Promise<void>;
  startNewConversation: (projectId?: string) => Promise<void>;
  dismissRestoreError: () => void;
  setTestMode: (mode: boolean) => void;
  handleEngage: () => Promise<void>;
  handleEnd: () => Promise<void>;
  handlePause: () => Promise<void>;
  handleResume: () => Promise<void>;
}

const VoiceSessionContext = createContext<VoiceSessionContextValue | null>(null);

function storeApi() {
  return useSessionStore.getState();
}

export const VoiceSessionProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const interactionState = useSessionStore((s) => s.interactionState);
  const interactionMode = useSessionStore((s) => s.interactionMode);
  const pipelineMode = useSessionStore((s) => s.pipelineMode);
  const isLaunching = useSessionStore((s) => s.isLaunching);
  const cpuWarning = useSessionStore((s) => s.cpuWarning);
  const testMode = useSessionStore((s) => s.testMode);
  const testingClip = useSessionStore((s) => s.testingClip);
  const dialogueHistory = useSessionStore((s) => s.dialogueHistory);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const isRestoring = useSessionStore((s) => s.isRestoring);
  const restoreError = useSessionStore((s) => s.restoreError);
  const restoreSignal = useSessionStore((s) => s.restoreSignal);
  const sessionListVersion = useSessionStore((s) => s.sessionListVersion);

  const { transcript, assistantText } = useTranscriptStream();

  const isEngaged = interactionState !== "Idle";
  const isSleeping = interactionState === "Sleeping";
  const isPaused = interactionState === "Paused";
  const isThinking = interactionState === "Thinking";
  const pttStatus: "IDLE" | "RECORDING" | "PROCESSING" =
    interactionMode === "PTT" && isEngaged
      ? interactionState === "Listening"
        ? "RECORDING"
        : interactionState === "Thinking"
          ? "PROCESSING"
          : "IDLE"
      : "IDLE";

  const handleStateChanged = useCallback((payload: StateChangedPayload) => {
    storeApi().setInteractionState(payload.state as InteractionState);
    const next = payload.state as InteractionState;
    if (next === "Ready" || next === "Idle") {
      storeApi().setTestingClip(null);
    }
  }, []);

  const handlePartial = useCallback(() => {}, []);
  const handleFinal = useCallback(() => {}, []);
  const handleToken = useCallback(() => {}, []);

  const handleSettingsUpdated = useCallback(() => {
    void (async () => {
      try {
        const res = await getSettings();
        const mode = res.settings?.interaction?.mode;
        const pmode = res.settings?.interaction?.pipeline_mode;
        if (mode) {
          storeApi().setInteractionMode(normalizeToInteractionModeUpper(mode));
        }
        if (pmode) {
          storeApi().setPipelineMode(pmode.toLowerCase() as "modular" | "realtime");
        }
      } catch {
        // Best-effort; settings sync must never break the session.
      }
    })();
  }, []);

  // Listeners are registered synchronously on mount, before any await.
  useSessionEvents({
    onStateChanged: handleStateChanged,
    onTranscriptPartial: handlePartial,
    onTranscriptFinal: handleFinal,
    onLlmToken: handleToken,
    onSettingsUpdated: handleSettingsUpdated,
  });

  const handleHydrated = useCallback(
    (result: {
      interactionMode: string | null;
      pipelineMode: "modular" | "realtime" | null;
      cpuWarning: { governor: string } | null;
      activeSessionId: number | null;
      dialogueHistory: DialogueTurn[];
      turnIdCounter: number;
    }) => {
      const api = storeApi();
      if (result.interactionMode) {
        api.setInteractionMode(normalizeToInteractionModeUpper(result.interactionMode));
      }
      if (result.pipelineMode) {
        api.setPipelineMode(result.pipelineMode);
      }
      api.setCpuWarning(result.cpuWarning);
      api.setActiveSessionId(result.activeSessionId);
      if (result.dialogueHistory.length > 0) {
        api.setDialogueHistory(result.dialogueHistory);
        api.setTurnIdCounter(result.turnIdCounter);
      }
    },
    [],
  );

  // Hydration runs after subscription is established.
  useSessionHydration(handleHydrated);

  const engage = useCallback(async () => {
    const api = storeApi();
    api.setIsLaunching(true);
    api.setSessionError(null);
    api.setTranscript("");
    api.setAssistantText("");
    try {
      const result = await engageSession((state) => storeApi().setInteractionState(state));
      if (!result.success) {
        api.setSessionError(
          result.reason === "timeout"
            ? "Session start timed out. State resynced from snapshot."
            : "Could not start session.",
        );
      }
    } catch {
      storeApi().setSessionError("Could not start session.");
    } finally {
      storeApi().setIsLaunching(false);
    }
  }, []);

  const disengage = useCallback(async () => {
    const api = storeApi();
    api.setIsLaunching(true);
    api.setTranscript("");
    api.setAssistantText("");
    api.setDialogueHistory([]);
    api.setActiveSessionId(null);
    api.setTurnIdCounter(0);
    api.bumpSessionListVersion();
    try {
      await disengageSession((state) => storeApi().setInteractionState(state));
    } catch {
      // End is best-effort; local state was already cleared.
    } finally {
      storeApi().setIsLaunching(false);
    }
  }, []);

  const pause = useCallback(async () => {
    const current = storeApi().interactionState;
    if (current === "Idle" || current === "Paused" || current === "Error") return;
    try {
      await pauseSession();
    } catch {
      // State converges via state_changed or snapshot reconcile.
    }
  }, []);

  const resume = useCallback(async () => {
    const current = storeApi().interactionState;
    if (current !== "Paused" && current !== "Error") return;
    try {
      await resumeSession();
    } catch {
      // State converges via state_changed or snapshot reconcile.
    }
  }, []);

  const handlePttStart = useCallback(async () => {
    const s = storeApi();
    if (s.interactionState === "Idle" || s.interactionState === "Paused" || s.interactionState === "Error") return;
    try {
      await pttStart();
    } catch {
      // No-op; PTT is best-effort per press.
    }
  }, []);

  const handlePttStop = useCallback(async () => {
    const s = storeApi();
    if (s.interactionState === "Idle" || s.interactionState === "Paused" || s.interactionState === "Error") return;
    try {
      await pttStop();
    } catch {
      // No-op; PTT is best-effort per press.
    }
  }, []);

  const handlePttCancel = useCallback(async () => {
    if (storeApi().interactionState === "Idle") return;
    try {
      await pttCancel();
    } catch {
      // No-op.
    }
  }, []);

  const togglePtt = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    if (pttStatus === "IDLE") {
      await handlePttStart();
    } else {
      await handlePttStop();
    }
  }, [isEngaged, isPaused, pttStatus, handlePttStart, handlePttStop]);

  const handleTestClip = useCallback(
    async (clipId: string) => {
      if (isEngaged) return;
      const api = storeApi();
      api.setTestingClip(clipId);
      api.setTestMode(false);
      api.setTranscript("");
      api.setAssistantText("");
      try {
        await testClip(clipId);
      } catch {
        storeApi().setTestingClip(null);
      }
    },
    [isEngaged],
  );

  const selectSession = useCallback(async (sessionId: number) => {
    const s = storeApi();
    if (sessionId === s.activeSessionId || s.isRestoring) return;
    storeApi().setIsRestoring(true);
    try {
      const result = await continueSessionIpc(sessionId);
      const history: DialogueTurn[] = result.turns.map((t) => ({
        user: t.user_text,
        assistant: t.assistant_text,
        id: t.turn_id,
      }));
      const api = storeApi();
      api.setDialogueHistory(history);
      api.setTurnIdCounter(history.reduce((max, h) => Math.max(max, h.id), 0));
      api.setActiveSessionId(sessionId);
      api.setRestoreSignal(api.restoreSignal + 1);
      api.bumpSessionListVersion();
    } catch (err: unknown) {
      storeApi().setRestoreError(err instanceof Error ? err.message : SESSION_COPY.restoreFailedFallback);
    } finally {
      storeApi().setIsRestoring(false);
    }
  }, []);

  const startNewConversation = useCallback(async (projectId?: string) => {
    if (storeApi().isRestoring) return;
    const api = storeApi();
    api.setTranscript("");
    api.setAssistantText("");
    api.setDialogueHistory([]);
    api.setTurnIdCounter(0);
    api.setActiveSessionId(null);
    api.setRestoreError(null);
    try {
      await createSessionIpc(projectId);
    } catch (err: unknown) {
      storeApi().setRestoreError(err instanceof Error ? err.message : SESSION_COPY.restoreFailedFallback);
    }
    storeApi().bumpSessionListVersion();
  }, []);

  const dismissRestoreError = useCallback(() => {
    storeApi().setRestoreError(null);
  }, []);

  const setTestMode = useCallback((mode: boolean) => {
    storeApi().setTestMode(mode);
  }, []);

  const value = useMemo<VoiceSessionContextValue>(
    () => ({
      interactionState,
      interactionMode: interactionMode as InteractionMode,
      pipelineMode,
      isEngaged,
      isSleeping,
      isPaused,
      isThinking,
      isLaunching,
      transcript,
      assistantText,
      cpuWarning,
      testMode,
      testingClip,
      dialogueHistory,
      activeSessionId,
      isRestoring,
      restoringSessionId: null,
      restoreError,
      restoreSignal,
      sessionListVersion,
      hasCachedSession: false,
      pttStatus,
      engage,
      disengage,
      pause,
      resume,
      handlePttStart,
      handlePttStop,
      handlePttCancel,
      togglePtt,
      handleTestClip,
      selectSession,
      startNewConversation,
      dismissRestoreError,
      setTestMode,
      handleEngage: engage,
      handleEnd: disengage,
      handlePause: pause,
      handleResume: resume,
    }),
    [
      interactionState,
      interactionMode,
      pipelineMode,
      isEngaged,
      isSleeping,
      isPaused,
      isThinking,
      isLaunching,
      transcript,
      assistantText,
      cpuWarning,
      testMode,
      testingClip,
      dialogueHistory,
      activeSessionId,
      isRestoring,
      restoreError,
      restoreSignal,
      sessionListVersion,
      pttStatus,
      engage,
      disengage,
      pause,
      resume,
      handlePttStart,
      handlePttStop,
      handlePttCancel,
      togglePtt,
      handleTestClip,
      selectSession,
      startNewConversation,
      dismissRestoreError,
      setTestMode,
    ],
  );

  return <VoiceSessionContext.Provider value={value}>{children}</VoiceSessionContext.Provider>;
};

export function useVoiceSession(): VoiceSessionContextValue {
  const ctx = useContext(VoiceSessionContext);
  if (!ctx) throw new Error("useVoiceSession must be used within VoiceSessionProvider");
  return ctx;
}
