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
  type VoiceErrorPayload,
  onStateChanged,
  onTranscriptPartial,
  onTranscriptFinal,
  onLlmToken,
  onVoiceError,
  onSettingsUpdated,
} from "@/services/eventsService";
import { getSettings } from "@/services/settingsService";
import { getTurns } from "@/services/historyService";

export type InteractionMode = "PASSIVE" | "PTT";

export interface DialogueTurn {
  user: string;
  assistant: string;
  id: number;
}

export interface VoiceSessionContextValue {
  // State
  interactionState: InteractionState;
  setInteractionState: (state: InteractionState) => void;
  interactionMode: InteractionMode;
  setInteractionMode: (mode: InteractionMode) => void;
  pipelineMode: "modular" | "realtime";
  setPipelineMode: (mode: "modular" | "realtime") => void;
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
  setDialogueHistory: React.Dispatch<React.SetStateAction<DialogueTurn[]>>;
  errorAlert: string | null;
  setErrorAlert: (error: string | null) => void;
  isThinking: boolean;

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
  dismissError: () => void;
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
  const isSleeping = interactionState === "Paused";
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
  const [errorAlert, setErrorAlert] = useState<string | null>(null);

  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const turnIdCounter = useRef(0);
  const hasActiveTurnStarted = useRef(false);
  const isSpacePressedRef = useRef(false);

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
    } catch (err: any) {
      console.error("[VoiceSession] Start session failed:", err);
      setErrorAlert(err?.message || "Voice engagement failed");
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
      setErrorAlert(err?.message || "Ending session failed");
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
      setErrorAlert(err?.message || "Pausing voice pipeline failed");
    }
  }, [interactionState]);

  const resume = useCallback(async () => {
    if (interactionState !== "Paused" && interactionState !== "Error") return;
    try {
      await resumeSession();
      setErrorAlert(null);
      // State transition to Ready is exclusively handled by onStateChanged IPC event
    } catch (err: any) {
      console.error("[VoiceSession] Resume failed:", err);
      setErrorAlert(err?.message || "Resuming voice pipeline failed");
    }
  }, [interactionState]);

  const handlePttStart = useCallback(async () => {
    if (!isEngaged || isPaused || interactionState === "Error") return;
    archiveCurrentTurn();
    try {
      await pttStart();
    } catch (err: any) {
      console.error("[VoiceSession] PTT start failed:", err);
      setErrorAlert(err?.message || "PTT start failed");
    }
  }, [isEngaged, isPaused, interactionState, archiveCurrentTurn]);

  const handlePttStop = useCallback(async () => {
    if (!isEngaged || isPaused || interactionState === "Error") return;
    try {
      await pttStop();
    } catch (err: any) {
      console.error("[VoiceSession] PTT stop failed:", err);
      setErrorAlert(err?.message || "PTT stop failed");
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
      setInteractionState("Ready");
    } catch (err) {
      console.error("[VoiceSession] Test clip failed:", err);
      setTestingClip(null);
    }
  }, [isEngaged, archiveCurrentTurn]);

  const clearHistory = useCallback(() => {
    setDialogueHistory([]);
  }, []);

  const dismissError = useCallback(() => {
    setErrorAlert(null);
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

        let partialThrottleTimer: ReturnType<typeof setTimeout> | null = null;
        let tokenThrottleTimer: ReturnType<typeof setTimeout> | null = null;

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
          onVoiceError((payload: VoiceErrorPayload) => {
            if (!isMounted) return;
            const msg = payload?.message || String(payload);
            setErrorAlert(msg);
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
      unlisteners.forEach((fn) => fn());
    };
  }, [archiveCurrentTurn]);

  const value: VoiceSessionContextValue = useMemo(
    () => ({
      interactionState,
      setInteractionState,
      interactionMode,
      setInteractionMode,
      pipelineMode,
      setPipelineMode,
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
      setDialogueHistory,
      errorAlert,
      setErrorAlert,
      isThinking,
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
      dismissError,
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
      errorAlert,
      isThinking,
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
      dismissError,
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
