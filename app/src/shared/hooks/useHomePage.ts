import { useState, useEffect, useRef, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  engage,
  startRealtimeSession,
  stopRealtimeSession,
  pausePipeline,
  resumePipeline,
  pttStart,
  pttStop,
  testClip,
  testClipCancel,
  getRealtimeSessionCache,
  getRuntimeSnapshot,
} from "@/services/pipelineService";
import { showMainWindow } from "@/services/windowService";
import { type InteractionState } from "@/services/eventsService";
import { getSettings } from "@/services/settingsService";
import { getTurns } from "@/services/historyService";
import { useTelemetry } from "@/shared/hooks/useTelemetry";

export type InteractionMode = "PASSIVE" | "PTT";
export type AmbientMood = "calm" | "active" | "thinking" | "speaking";

export function toMood(state: InteractionState, sleeping: boolean): AmbientMood {
  if (sleeping) return "calm";
  switch (state) {
    case "UserSpeaking":
      return "active";
    case "Thinking":
      return "thinking";
    case "AssistantSpeaking":
      return "speaking";
    case "Listening":
      return "active";
    default:
      return "calm";
  }
}

export function toStatusLabel(
  state: InteractionState,
  engaged: boolean,
  sleeping: boolean,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  isPaused: boolean,
  idleTimeout: number | null
): string {
  if (!engaged) return "Dormant";
  if (isPaused) return "Paused";
  if (idleTimeout !== null) return `Idle · ${idleTimeout}s`;
  if (sleeping) return "Sleeping";
  if (ptt === "RECORDING") return "Recording";
  if (ptt === "PROCESSING") return "Processing";
  switch (state) {
    case "UserSpeaking":
      return "Listening";
    case "Thinking":
      return "Thinking";
    case "AssistantSpeaking":
      return "Speaking";
    case "Listening":
      return "Ready";
    case "Interrupted":
      return "Interrupted";
    default:
      return "Ready";
  }
}

export function isDotActive(
  engaged: boolean,
  state: InteractionState,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  sleeping: boolean
): boolean {
  if (!engaged || sleeping) return false;
  if (ptt === "RECORDING" || ptt === "PROCESSING") return true;
  return state === "UserSpeaking" || state === "Thinking" || state === "AssistantSpeaking";
}

export function useHomePage() {
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("PASSIVE");
  const [pipelineMode, setPipelineMode] = useState<"modular" | "realtime">("modular");
  const [isEngaged, setIsEngaged] = useState(false);
  const [isSleeping, setIsSleeping] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [hasCachedSession, setHasCachedSession] = useState(false);
  const [pttStatus, setPttStatus] = useState<"IDLE" | "RECORDING" | "PROCESSING">("IDLE");
  const [isLaunching, setIsLaunching] = useState(false);

  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [cpuWarning, setCpuWarning] = useState<{ governor: string } | null>(null);
  const [idleTimeout, setIdleTimeout] = useState<number | null>(null);

  const [testMode, setTestMode] = useState(false);
  const [testingClip, setTestingClip] = useState<string | null>(null);

  const [dialogueHistory, setDialogueHistory] = useState<Array<{ user: string; assistant: string; id: number }>>([]);
  const [historyOpen, setHistoryOpen] = useState(false);

  const [isMobileScreen, setIsMobileScreen] = useState(
    typeof window !== "undefined" ? window.innerWidth < 768 : false
  );

  useEffect(() => {
    const checkMobile = () => setIsMobileScreen(window.innerWidth < 768);
    checkMobile();
    window.addEventListener("resize", checkMobile);
    return () => window.removeEventListener("resize", checkMobile);
  }, []);

  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const turnIdCounter = useRef(0);
  const hasActiveTurnStarted = useRef(false);
  const isSpacePressedRef = useRef(false);
  const telemetryRef = useTelemetry();
  const [errorAlert, setErrorAlert] = useState<string | null>(null);
  const dialogueScrollRef = useRef<HTMLDivElement>(null);
  const prevEngagedRef = useRef(false);
  const testButtonRef = useRef<HTMLButtonElement>(null);
  const testPanelRef = useRef<HTMLDivElement>(null);

  const isThinking = interactionState === "Thinking" || pttStatus === "PROCESSING";

  const archiveCurrentTurn = useCallback(() => {
    const userText = activeUserTextRef.current.trim();
    const aiText = activeAiTextRef.current.trim();
    if (userText || aiText) {
      turnIdCounter.current += 1;
      const newTurn = {
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

  const handleEngage = useCallback(async () => {
    archiveCurrentTurn();
    hasActiveTurnStarted.current = false;
    setIsLaunching(true);
    setIsEngaged(true);
    setIsSleeping(false);
    setIsPaused(false);
    try {
      if (pipelineMode === "realtime") {
        await startRealtimeSession();
      } else {
        await engage();
      }
    } catch (err: any) {
      console.error("[Home] Engage failed:", err);
      setErrorAlert(err?.message || "Voice engagement failed");
      setIsEngaged(false);
    } finally {
      setIsLaunching(false);
    }
  }, [pipelineMode, archiveCurrentTurn]);

  const handleEnd = useCallback(async () => {
    archiveCurrentTurn();
    hasActiveTurnStarted.current = false;
    setIsLaunching(true);
    setIsEngaged(false);
    setIsSleeping(false);
    setIsPaused(false);
    setTranscript("");
    setAssistantText("");
    
    const wasTesting = !!testingClip;
    setTestingClip(null);

    try {
      if (wasTesting) {
        await testClipCancel();
      } else if (pipelineMode === "realtime") {
        await stopRealtimeSession();
      } else {
        await engage();
      }
    } catch (err: any) {
      console.error("[Home] End session failed:", err);
      setErrorAlert(err?.message || "Ending session failed");
    } finally {
      setIsLaunching(false);
    }
  }, [pipelineMode, testingClip, archiveCurrentTurn]);

  const handlePause = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    setIsPaused(true);
    try {
      await pausePipeline();
    } catch (err: any) {
      console.error("[Home] Pause failed:", err);
      setErrorAlert(err?.message || "Pausing voice pipeline failed");
      setIsPaused(false);
    }
  }, [isEngaged, isPaused]);

  const handleResume = useCallback(async () => {
    if (!isEngaged || !isPaused) return;
    setIsPaused(false);
    try {
      await resumePipeline();
    } catch (err: any) {
      console.error("[Home] Resume failed:", err);
      setErrorAlert(err?.message || "Resuming voice pipeline failed");
      setIsPaused(true);
    }
  }, [isEngaged, isPaused]);

  const togglePtt = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    try {
      if (pttStatus === "IDLE") {
        archiveCurrentTurn();
        await pttStart("MainWindow");
      } else {
        await pttStop("MainWindow");
      }
    } catch (err: any) {
      console.error("[Home] PTT toggle failed:", err);
      setErrorAlert(err?.message || "PTT toggle failed");
    }
  }, [isEngaged, isPaused, pttStatus, archiveCurrentTurn]);

  const handleTestClip = useCallback(async (clipId: string) => {
    if (isEngaged) return;
    archiveCurrentTurn();
    hasActiveTurnStarted.current = false;
    setTestingClip(clipId);
    setIsEngaged(true);
    setTestMode(false);
    setTranscript("");
    setAssistantText("");
    try {
      await testClip(clipId);
    } catch (err) {
      console.error("[Home] Test clip failed:", err);
      setTestingClip(null);
      setIsEngaged(false);
    }
  }, [isEngaged, archiveCurrentTurn]);

  // Keyboard Shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const key = e.key.toLowerCase();
      if (e.code === "Space") {
        if (interactionMode === "PTT" && isEngaged && !isPaused) {
          e.preventDefault();
          if (!isSpacePressedRef.current) {
            isSpacePressedRef.current = true;
            archiveCurrentTurn();
            pttStart("MainWindow").catch((err) => {
              console.error("[Home] PTT start failed:", err);
              isSpacePressedRef.current = false;
            });
          }
        }
      } else if (key === "s") {
        e.preventDefault();
        if (isEngaged) handleEnd();
        else handleEngage();
      } else if (key === "p") {
        e.preventDefault();
        handlePause();
      } else if (key === "r") {
        e.preventDefault();
        handleResume();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space" && isSpacePressedRef.current) {
        isSpacePressedRef.current = false;
        if (interactionMode === "PTT" && isEngaged && !isPaused) {
          e.preventDefault();
          pttStop("MainWindow").catch((err) => console.error("[Home] PTT stop failed:", err));
        }
      }
    };

    const handleBlur = () => {
      if (isSpacePressedRef.current && interactionMode === "PTT" && isEngaged && !isPaused) {
        isSpacePressedRef.current = false;
        pttStop("MainWindow").catch(() => {});
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleBlur);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleBlur);
    };
  }, [isEngaged, isPaused, interactionMode, handleEngage, handleEnd, handlePause, handleResume, archiveCurrentTurn]);

  // Session Cache Check
  useEffect(() => {
    if (pipelineMode === "realtime" && !isEngaged) {
      getRealtimeSessionCache()
        .then((cache) => setHasCachedSession(cache?.has_session ?? false))
        .catch((err) => console.warn("[Home] Failed to check session cache:", err));
    } else {
      setHasCachedSession(false);
    }
  }, [isEngaged, pipelineMode]);

  // Tauri Event Listeners (Safely registered concurrently with memory leak guards)
  useEffect(() => {
    let isMounted = true;
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        const settings = await getSettings();
        if (!isMounted) return;

        if (settings?.interaction?.main_app_mode) {
          setInteractionMode(settings.interaction.main_app_mode.toUpperCase() as InteractionMode);
        }
        if (settings?.interaction?.pipeline_mode) {
          setPipelineMode(settings.interaction.pipeline_mode.toLowerCase() as "modular" | "realtime");
        }

        try {
          const snapshot = await getRuntimeSnapshot();
          if (snapshot && isMounted) {
            const engaged = snapshot.is_engaged ?? false;
            setIsEngaged(engaged);
            prevEngagedRef.current = engaged;
            setIsSleeping(snapshot.is_sleeping ?? false);
            if (snapshot.cpu_governor && !snapshot.cpu_governor_optimal) {
              setCpuWarning({ governor: snapshot.cpu_governor });
            }

            if (engaged && snapshot.conversation_id && snapshot.conversation_id !== 0) {
              const turns = await getTurns(snapshot.conversation_id);
              if (isMounted) {
                const history = turns.map((t) => ({
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
          console.warn("[Home] Failed to sync initial state:", e);
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
              }
              if (newState === "UserSpeaking" || newState === "Listening") {
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
              activeAiTextRef.current = event.payload;
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
            "ptt_status",
            (event) => {
              if (!isMounted) return;
              setPttStatus(event.payload.state as "IDLE" | "RECORDING" | "PROCESSING");
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
              setIsSleeping(event.payload);
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
            console.error(`[Home] Failed to listen to ${event}:`, err);
          }
        });

        await Promise.all(listenPromises);

        setTimeout(async () => {
          if (isMounted) await showMainWindow();
        }, 300);
      } catch (err) {
        console.error("[Home] Failed to setup listeners:", err);
      }
    };

    setup();
    return () => {
      isMounted = false;
      unlisteners.forEach((fn) => fn());
    };
  }, [archiveCurrentTurn]);

  return {
    interactionState,
    interactionMode,
    pipelineMode,
    isEngaged,
    isSleeping,
    isPaused,
    hasCachedSession,
    pttStatus,
    transcript,
    assistantText,
    cpuWarning,
    idleTimeout,
    testMode,
    setTestMode,
    testingClip,
    dialogueHistory,
    historyOpen,
    setHistoryOpen,
    telemetryRef,
    errorAlert,
    setErrorAlert,
    dialogueScrollRef,
    isLaunching,
    isThinking,
    isMobileScreen,
    testButtonRef,
    testPanelRef,
    handleEngage,
    handleEnd,
    handlePause,
    handleResume,
    togglePtt,
    handleTestClip,
  };
}
