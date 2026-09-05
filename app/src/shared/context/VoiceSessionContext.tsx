import React, { createContext, useContext, useState, useEffect, useRef, useCallback, useMemo, ReactNode } from "react";
import {
  startSession,
  endSession,
  pauseSession,
  resumeSession,
  pttStart,
  pttStop,
  pttCancel,
  testClip,
  testClipCancel,
  getRuntimeSnapshot,
} from "@/services/pipelineService";
import {
  type InteractionState,
  type StateChangedPayload,
  type TranscriptPayload,
  type LlmTokenPayload,
  onStateChanged,
  onTranscriptPartial,
  onTranscriptFinal,
  onLlmToken,
  onSettingsUpdated,
} from "@/services/eventsService";
import { getSettings } from "@/services/settingsService";
import {
  getTurns,
  selectSession as selectSessionIpc,
  startNewConversation as startNewConversationIpc,
} from "@/services/historyService";
import { SESSION_COPY } from "@/data/sessionCopy";

export type InteractionMode = "PASSIVE" | "PTT";

export interface DialogueTurn {
  user: string;
  assistant: string;
  id: number;
}

export interface VoiceSessionContextValue {
  // State
  interactionState: InteractionState;
  interactionMode: InteractionMode;
  pipelineMode: "modular" | "realtime";
  isEngaged: boolean;
  isSleeping: boolean;
  isPaused: boolean;
  hasCachedSession: boolean;
  pttStatus: "IDLE" | "RECORDING" | "PROCESSING";
  isLaunching: boolean;
  transcript: string;
  assistantText: string;
  cpuWarning: { governor: string } | null;
  testMode: boolean;
  setTestMode: (mode: boolean) => void;
  testingClip: string | null;
  dialogueHistory: DialogueTurn[];
  isThinking: boolean;

  // Session continuation (§A/B)
  activeSessionId: number | null;
  isRestoring: boolean;
  restoringSessionId: number | null;
  restoreError: string | null;
  restoreSignal: number;
  sessionListVersion: number;
  selectSession: (sessionId: number) => Promise<void>;
  startNewConversation: () => Promise<void>;
  dismissRestoreError: () => void;

  // Discrete UI Actions
  engage: () => Promise<void>;
  disengage: () => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;

  // PTT Actions
  handlePttStart: () => Promise<void>;
  handlePttStop: () => Promise<void>;
  handlePttCancel: () => Promise<void>;

  // Aliases for compatibility
  handleEngage: () => Promise<void>;
  handleEnd: () => Promise<void>;
  handlePause: () => Promise<void>;
  handleResume: () => Promise<void>;
  togglePtt: () => Promise<void>;

  // Clip Testing & Reset
  handleTestClip: (clipId: string) => Promise<void>;
  clearHistory: () => void;
}

const VoiceSessionContext = createContext<VoiceSessionContextValue | null>(null);

export const VoiceSessionProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("PASSIVE");
  const [pipelineMode, setPipelineMode] = useState<"modular" | "realtime">("modular");
  const [hasCachedSession] = useState(false);
  const [isLaunching, setIsLaunching] = useState(false);

  // Pure derived state from the canonical source of truth (interactionState)
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

  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [cpuWarning, setCpuWarning] = useState<{ governor: string } | null>(null);

  const [testMode, setTestMode] = useState(false);
  const [testingClip, setTestingClip] = useState<string | null>(null);
  const [dialogueHistory, setDialogueHistory] = useState<DialogueTurn[]>([]);

  // Session continuation state (§A/B): which persisted session subsequent
  // turns append to, restore lifecycle, and rail list versioning.
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [isRestoring, setIsRestoring] = useState(false);
  const [restoringSessionId, setRestoringSessionId] = useState<number | null>(null);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoreSignal, setRestoreSignal] = useState(0);
  const [sessionListVersion, setSessionListVersion] = useState(0);

  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const turnIdCounter = useRef(0);
  const hasActiveTurnStarted = useRef(false);
  const isSpacePressedRef = useRef(false);
  const activeSessionIdRef = useRef<number | null>(null);
  activeSessionIdRef.current = activeSessionId;
  const restoringRef = useRef(false);

  const archiveCurrentTurn = useCallback(() => {
    const userText = activeUserTextRef.current.trim();
    const aiText = activeAiTextRef.current.trim();
    if (userText || aiText) {
      turnIdCounter.current += 1;
      const newTurn: DialogueTurn = {
        user: userText,
        assistant: aiText,
        id: turnIdCounter.current,
      };
      setDialogueHistory((prev) => {
        const next = [...prev, newTurn];
        return next.length > 100 ? next.slice(next.length - 100) : next;
      });
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
      setTranscript("");
      setAssistantText("");
    }
  }, []);

  const engage = useCallback(async () => {
    archiveCurrentTurn();
    hasActiveTurnStarted.current = false;
    setIsLaunching(true);
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setTranscript("");
    setAssistantText("");
    try {
      await startSession();
      // State transition to Ready is exclusively handled by onStateChanged IPC event
      try {
        const snapshot = await getRuntimeSnapshot();
        if (snapshot && snapshot.conversation_id !== 0) {
          setActiveSessionId(snapshot.conversation_id);
        }
      } catch {
        // Active-session highlight is best-effort; the session still works.
      } finally {
        setSessionListVersion((v) => v + 1);
      }
    } catch (err: any) {
      console.error("[VoiceSession] Start session failed:", err);
    } finally {
      setIsLaunching(false);
    }
  }, [archiveCurrentTurn]);

  const disengage = useCallback(async () => {
    hasActiveTurnStarted.current = false;
    setIsLaunching(true);
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setTranscript("");
    setAssistantText("");
    // Transcripts tray clear on End per approved specification
    setDialogueHistory([]);
    turnIdCounter.current = 0;
    setActiveSessionId(null);
    setSessionListVersion((v) => v + 1);

    const wasTesting = !!testingClip;
    setTestingClip(null);

    try {
      if (wasTesting) {
        await testClipCancel();
      } else {
        await endSession();
      }
      // State transition to Idle is exclusively handled by onStateChanged IPC event
    } catch (err: any) {
      console.error("[VoiceSession] End session failed:", err);
    } finally {
      setIsLaunching(false);
    }
  }, [testingClip]);

  const pause = useCallback(async () => {
    if (interactionState === "Idle" || interactionState === "Paused" || interactionState === "Error") return;
    try {
      await pauseSession();
      // State transition to Paused is exclusively handled by onStateChanged IPC event
    } catch (err: any) {
      console.error("[VoiceSession] Pause failed:", err);
    }
  }, [interactionState]);

  const resume = useCallback(async () => {
    if (interactionState !== "Paused" && interactionState !== "Error") return;
    try {
      await resumeSession();
      // State transition to Ready is exclusively handled by onStateChanged IPC event
    } catch (err: any) {
      console.error("[VoiceSession] Resume failed:", err);
    }
  }, [interactionState]);

  const handlePttStart = useCallback(async () => {
    if (!isEngaged || isPaused || interactionState === "Error") return;
    archiveCurrentTurn();
    try {
      await pttStart();
    } catch (err: any) {
      console.error("[VoiceSession] PTT start failed:", err);
    }
  }, [isEngaged, isPaused, interactionState, archiveCurrentTurn]);

  const handlePttStop = useCallback(async () => {
    if (!isEngaged || isPaused || interactionState === "Error") return;
    try {
      await pttStop();
    } catch (err: any) {
      console.error("[VoiceSession] PTT stop failed:", err);
    }
  }, [isEngaged, isPaused, interactionState]);

  const handlePttCancel = useCallback(async () => {
    if (!isEngaged) return;
    try {
      await pttCancel();
    } catch (err: any) {
      console.error("[VoiceSession] PTT cancel failed:", err);
    }
  }, [isEngaged]);

  const togglePtt = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    if (pttStatus === "IDLE") {
      await handlePttStart();
    } else {
      await handlePttStop();
    }
  }, [isEngaged, isPaused, pttStatus, handlePttStart, handlePttStop]);

  const handleTestClip = useCallback(async (clipId: string) => {
    if (isEngaged) return;
    archiveCurrentTurn();
    hasActiveTurnStarted.current = false;
    setTestingClip(clipId);
    setTestMode(false);
    setTranscript("");
    setAssistantText("");
    try {
      await testClip(clipId);
    } catch (err) {
      console.error("[VoiceSession] Test clip failed:", err);
      setTestingClip(null);
    }
  }, [isEngaged, archiveCurrentTurn]);

  const clearHistory = useCallback(() => {
    setDialogueHistory([]);
  }, []);

  const dismissRestoreError = useCallback(() => {
    setRestoreError(null);
  }, []);

  /**
   * Restore a persisted session and make it active (spec §B.7–8, §B.10).
   * Loads turns in persisted order with no extras; selecting the
   * already-active session is a no-op (no reload, no reset, no animation).
   * Late live buffers from the previous session are discarded, never
   * archived into the restored transcript (spec §B.9 frontend half).
   */
  const selectSession = useCallback(async (sessionId: number) => {
    if (sessionId === activeSessionIdRef.current || restoringRef.current) return;
    restoringRef.current = true;
    setIsRestoring(true);
    setRestoringSessionId(sessionId);
    setRestoreError(null);
    try {
      const turns = await selectSessionIpc(sessionId);
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
      setTranscript("");
      setAssistantText("");
      const history: DialogueTurn[] = turns.map((t) => ({
        user: t.user_text,
        assistant: t.assistant_text,
        id: t.turn_id,
      }));
      turnIdCounter.current = history.reduce((max, h) => Math.max(max, h.id), 0);
      setDialogueHistory(history);
      setActiveSessionId(sessionId);
      setRestoreSignal((s) => s + 1);
      setSessionListVersion((v) => v + 1);
    } catch (err: unknown) {
      console.error("[VoiceSession] Restore session failed:", err);
      setRestoreError(
        err instanceof Error ? err.message : SESSION_COPY.restoreFailedFallback
      );
    } finally {
      restoringRef.current = false;
      setIsRestoring(false);
      setRestoringSessionId(null);
    }
  }, []);

  /**
   * Start an explicit new conversation (spec §A.5, §B.16): empty session
   * with no inherited turns — and no restore animation.
   */
  const startNewConversation = useCallback(async () => {
    if (restoringRef.current) return;
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setTranscript("");
    setAssistantText("");
    setDialogueHistory([]);
    turnIdCounter.current = 0;
    setActiveSessionId(null);
    setRestoreError(null);
    try {
      await startNewConversationIpc();
    } catch (err: unknown) {
      console.error("[VoiceSession] New conversation failed:", err);
      setRestoreError(
        err instanceof Error ? err.message : SESSION_COPY.restoreFailedFallback
      );
    } finally {
      setSessionListVersion((v) => v + 1);
    }
  }, []);

  // Keyboard PTT integration
  useEffect(() => {
    if (interactionMode !== "PTT" || !isEngaged || isPaused) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Space" && !e.repeat && !isSpacePressedRef.current) {
        const target = e.target as HTMLElement;
        if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;
        e.preventDefault();
        isSpacePressedRef.current = true;
        handlePttStart();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space" && isSpacePressedRef.current) {
        const target = e.target as HTMLElement;
        if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;
        e.preventDefault();
        isSpacePressedRef.current = false;
        handlePttStop();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [interactionMode, isEngaged, isPaused, handlePttStart, handlePttStop]);

  // Initial Sync & Tauri Event Listeners
  useEffect(() => {
    let isMounted = true;
    const unlisteners: (() => void)[] = [];
    let partialThrottleTimer: ReturnType<typeof setTimeout> | null = null;
    let tokenThrottleTimer: ReturnType<typeof setTimeout> | null = null;

    const setup = async () => {
      try {
        const res = await getSettings();
        const settings = (res as any)?.settings ?? res;
        if (settings?.interaction?.mode) {
          setInteractionMode(settings.interaction.mode.toUpperCase() as InteractionMode);
        }
        if (settings?.interaction?.pipeline_mode) {
          setPipelineMode(settings.interaction.pipeline_mode.toLowerCase() as "modular" | "realtime");
        }

        try {
          const snapshot = await getRuntimeSnapshot();
          if (snapshot && isMounted) {
            if (snapshot.pipeline_state) {
              setInteractionState(snapshot.pipeline_state as InteractionState);
            }
            if (snapshot.cpu_governor && !snapshot.cpu_governor_optimal) {
              setCpuWarning({ governor: snapshot.cpu_governor });
            }

            if (snapshot.conversation_id && snapshot.conversation_id !== 0) {
              const turns = await getTurns(snapshot.conversation_id);
              if (isMounted) {
                setActiveSessionId(snapshot.conversation_id);
                const history: DialogueTurn[] = turns.slice(-100).map((t) => ({
                  user: t.user_text,
                  assistant: t.assistant_text,
                  id: t.turn_id,
                }));
                setDialogueHistory(history);
                if (history.length > 0) {
                  turnIdCounter.current = Math.max(...history.map((h) => h.id));
                }
              }
            }
          }
        } catch (e) {
          console.warn("[VoiceSession] Failed to sync initial state:", e);
        }

        unlisteners.push(
          onStateChanged((payload: StateChangedPayload) => {
            if (!isMounted) return;
            if (payload && payload.owner === "Dictation") {
              return;
            }
            const newState = payload.state as InteractionState;
            setInteractionState(newState);
            if (newState !== "Idle") {
              hasActiveTurnStarted.current = true;
            } else {
              activeUserTextRef.current = "";
              activeAiTextRef.current = "";
              setTranscript("");
              setAssistantText("");
            }
            if (newState === "Ready" || newState === "Idle") {
              setTestingClip(null);
            }
            if (newState === "Listening") {
              archiveCurrentTurn();
            }
          })
        );

        unlisteners.push(
          onTranscriptPartial((payload: TranscriptPayload) => {
            if (!isMounted) return;
            activeUserTextRef.current = payload.text;
            if (!partialThrottleTimer) {
              partialThrottleTimer = setTimeout(() => {
                if (isMounted) setTranscript(activeUserTextRef.current);
                partialThrottleTimer = null;
              }, 30);
            }
          })
        );

        unlisteners.push(
          onTranscriptFinal((payload: TranscriptPayload) => {
            if (!isMounted) return;
            if (partialThrottleTimer) {
              clearTimeout(partialThrottleTimer);
              partialThrottleTimer = null;
            }
            activeUserTextRef.current = payload.text;
            setTranscript(payload.text);
          })
        );

        unlisteners.push(
          onLlmToken((payload: LlmTokenPayload) => {
            if (!isMounted) return;
            activeAiTextRef.current += payload.token;
            if (!tokenThrottleTimer) {
              tokenThrottleTimer = setTimeout(() => {
                if (isMounted) setAssistantText(activeAiTextRef.current);
                tokenThrottleTimer = null;
              }, 30);
            }
          })
        );

        unlisteners.push(
          onSettingsUpdated(async () => {
            if (!isMounted) return;
            try {
              const b = await getSettings();
              const s = b?.settings;
              if (s?.interaction?.mode && isMounted) {
                setInteractionMode(s.interaction.mode.toUpperCase() as InteractionMode);
              }
              if (s?.interaction?.pipeline_mode && isMounted) {
                setPipelineMode(s.interaction.pipeline_mode.toLowerCase() as "modular" | "realtime");
              }
            } catch (e) {
              console.warn("[VoiceSession] Failed to reload settings on settings-updated:", e);
            }
          })
        );

        // Window reveal is now the sole responsibility of App.tsx (after
        // core:window:allow-show was added 2026-09-03). Previously this site
        // issued a second showMainWindow() 300ms after the App.tsx show as a
        // workaround, which caused a visible WM state-transition flash
        // (minimized -> maximized) on first launch.
      } catch (err) {
        console.error("[VoiceSession] Failed to setup listeners:", err);
      }
    };

    setup();
    return () => {
      isMounted = false;
      if (partialThrottleTimer) {
        clearTimeout(partialThrottleTimer);
        partialThrottleTimer = null;
      }
      if (tokenThrottleTimer) {
        clearTimeout(tokenThrottleTimer);
        tokenThrottleTimer = null;
      }
      unlisteners.forEach((fn) => fn());
    };
  }, [archiveCurrentTurn]);

  const value: VoiceSessionContextValue = useMemo(
    () => ({
      interactionState,
      interactionMode,
      pipelineMode,
      isEngaged,
      isSleeping,
      isPaused,
      hasCachedSession,
      pttStatus,
      isLaunching,
      transcript,
      assistantText,
      cpuWarning,
      testMode,
      setTestMode,
      testingClip,
      dialogueHistory,
      isThinking,
      activeSessionId,
      isRestoring,
      restoringSessionId,
      restoreError,
      restoreSignal,
      sessionListVersion,
      selectSession,
      startNewConversation,
      dismissRestoreError,
      engage,
      disengage,
      pause,
      resume,
      handlePttStart,
      handlePttStop,
      handlePttCancel,
      handleEngage: engage,
      handleEnd: disengage,
      handlePause: pause,
      handleResume: resume,
      togglePtt,
      handleTestClip,
      clearHistory,
    }),
    [
      interactionState,
      interactionMode,
      pipelineMode,
      isEngaged,
      isSleeping,
      isPaused,
      hasCachedSession,
      pttStatus,
      isLaunching,
      transcript,
      assistantText,
      cpuWarning,
      testMode,
      testingClip,
      dialogueHistory,
      isThinking,
      activeSessionId,
      isRestoring,
      restoringSessionId,
      restoreError,
      restoreSignal,
      sessionListVersion,
      selectSession,
      startNewConversation,
      dismissRestoreError,
      engage,
      disengage,
      pause,
      resume,
      handlePttStart,
      handlePttStop,
      handlePttCancel,
      togglePtt,
      handleTestClip,
      clearHistory,
    ]
  );

  return <VoiceSessionContext.Provider value={value}>{children}</VoiceSessionContext.Provider>;
};

export function useVoiceSession(): VoiceSessionContextValue {
  const ctx = useContext(VoiceSessionContext);
  if (!ctx) {
    throw new Error("useVoiceSession must be used within a VoiceSessionProvider");
  }
  return ctx;
}
