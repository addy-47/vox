import React, { createContext, useContext, useState, useEffect, useRef, useCallback, useMemo, ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
import { showMainWindow } from "@/services/windowService";
import { type InteractionState } from "@/services/eventsService";
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
  idleTimeout: number | null;
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
  const [idleTimeout, setIdleTimeout] = useState<number | null>(null);

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

    const wasTesting = !!testingClip;
    setTestingClip(null);

    try {
      if (wasTesting) {
        await testClipCancel();
      } else {
        await endSession();
      }
    } catch (err: any) {
      console.error("[VoiceSession] End session failed:", err);
      setErrorAlert(err?.message || "Ending session failed");
    } finally {
      setIsLaunching(false);
    }
  }, [testingClip]);

  const pause = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    setInteractionState("Paused");
    try {
      await pauseSession();
    } catch (err: any) {
      console.error("[VoiceSession] Pause failed:", err);
      setErrorAlert(err?.message || "Pausing voice pipeline failed");
      setInteractionState("Ready");
    }
  }, [isEngaged, isPaused]);

  const resume = useCallback(async () => {
    if (!isEngaged || !isPaused) return;
    setInteractionState("Ready");
    try {
      await resumeSession();
    } catch (err: any) {
      console.error("[VoiceSession] Resume failed:", err);
      setErrorAlert(err?.message || "Resuming voice pipeline failed");
      setInteractionState("Paused");
    }
  }, [isEngaged, isPaused]);

  const handlePttStart = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    archiveCurrentTurn();
    try {
      await pttStart();
    } catch (err: any) {
      console.error("[VoiceSession] PTT start failed:", err);
      setErrorAlert(err?.message || "PTT start failed");
    }
  }, [isEngaged, isPaused, archiveCurrentTurn]);

  const handlePttStop = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    try {
      await pttStop();
    } catch (err: any) {
      console.error("[VoiceSession] PTT stop failed:", err);
      setErrorAlert(err?.message || "PTT stop failed");
    }
  }, [isEngaged, isPaused]);

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
        const settings = await getSettings();
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

        const appWindow = getCurrentWindow();
        let partialThrottleTimer: ReturnType<typeof setTimeout> | null = null;
        let tokenThrottleTimer: ReturnType<typeof setTimeout> | null = null;

        const eventsList: Array<[string, (event: any) => void]> = [
          [
            "state_changed",
            (event) => {
              if (!isMounted) return;
              const newState = event.payload as InteractionState;
              setInteractionState(newState);
              if (newState !== "Idle") {
                hasActiveTurnStarted.current = true;
                setIdleTimeout(null);
              } else {
                activeUserTextRef.current = "";
                activeAiTextRef.current = "";
                setTranscript("");
                setAssistantText("");
              }
              if (newState === "Listening") {
                archiveCurrentTurn();
              }
            },
          ],
          [
            "transcript_partial",
            (event) => {
              if (!isMounted) return;
              activeUserTextRef.current = event.payload.text;
              setIdleTimeout(null);
              if (!partialThrottleTimer) {
                partialThrottleTimer = setTimeout(() => {
                  if (isMounted) setTranscript(activeUserTextRef.current);
                  partialThrottleTimer = null;
                }, 30);
              }
            },
          ],
          [
            "transcript_final",
            (event) => {
              if (!isMounted) return;
              if (partialThrottleTimer) {
                clearTimeout(partialThrottleTimer);
                partialThrottleTimer = null;
              }
              activeUserTextRef.current = event.payload.text;
              setTranscript(event.payload.text);
              setIdleTimeout(null);
            },
          ],
          [
            "llm_token",
            (event) => {
              if (!isMounted) return;
              activeAiTextRef.current = event.payload.token ?? event.payload.text ?? event.payload;
              setIdleTimeout(null);
              if (!tokenThrottleTimer) {
                tokenThrottleTimer = setTimeout(() => {
                  if (isMounted) setAssistantText(activeAiTextRef.current);
                  tokenThrottleTimer = null;
                }, 30);
              }
            },
          ],
          [
            "mode_changed_main",
            (event) => {
              if (!isMounted) return;
              setInteractionMode(event.payload.toUpperCase() as InteractionMode);
            },
          ],
          [
            "idle_timeout_tick",
            (event) => {
              if (!isMounted) return;
              setIdleTimeout(event.payload.seconds);
            },
          ],
          [
            "idle_timeout_reset",
            () => {
              if (!isMounted) return;
              setIdleTimeout(null);
            },
          ],
          [
            "hud_sleep_state",
            (event) => {
              if (!isMounted) return;
              if (event.payload) archiveCurrentTurn();
            },
          ],
          [
            "pipeline_mode_changed",
            (event) => {
              if (!isMounted) return;
              setPipelineMode(event.payload.toLowerCase() as "modular" | "realtime");
            },
          ],
        ];

        const listenPromises = eventsList.map(async ([event, handler]) => {
          try {
            const unlisten = await appWindow.listen(event, handler);
            if (!isMounted) {
              unlisten();
            } else {
              unlisteners.push(unlisten);
            }
          } catch (err) {
            console.error(`[VoiceSession] Failed to listen to ${event}:`, err);
          }
        });

        await Promise.all(listenPromises);

        setTimeout(async () => {
          if (isMounted) await showMainWindow();
        }, 300);
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
      idleTimeout,
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
      idleTimeout,
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
