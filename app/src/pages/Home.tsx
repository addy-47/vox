import React, { useState, useEffect, useRef , useCallback} from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { ErrorBoundary } from "@/shared/components/ErrorBoundary";
import { PipelineField } from "@/shared/components/PipelineField";
import { AmbientBackground } from "@/shared/components/AmbientBackground";
import { StatusCapsule } from "@/shared/components/StatusCapsule";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { Power, Mic, FlaskConical, Play, Pause, X, AlertCircle } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { cn } from "@/shared/lib/utils";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import ReactMarkdown from "react-markdown";

const MarkdownComponents = {
  h1: ({node, ...props}: any) => <h1 className="text-[12px] font-bold mt-1 mb-0.5" {...props} />,
  h2: ({node, ...props}: any) => <h2 className="text-[12px] font-bold mt-1 mb-0.5" {...props} />,
  h3: ({node, ...props}: any) => <h3 className="text-[11px] font-bold mt-1 mb-0.5" {...props} />,
  p: ({node, ...props}: any) => <p className="mb-1 last:mb-0 inline-block w-full" {...props} />,
  ul: ({node, ...props}: any) => <ul className="list-disc list-inside mb-1 pl-1" {...props} />,
  ol: ({node, ...props}: any) => <ol className="list-decimal list-inside mb-1 pl-1" {...props} />,
  li: ({node, ...props}: any) => <li className="ml-0" {...props} />,
  code: ({node, ...props}: any) => <code className="bg-[rgba(var(--foreground),0.06)] px-1 rounded font-mono text-[11px]" {...props} />,
};

// ─── Types ────────────────────────────────────────────────────────────────────

type InteractionState =
  | "Idle"
  | "Listening"
  | "UserSpeaking"
  | "Thinking"
  | "AssistantSpeaking"
  | "Interrupted";

type InteractionMode = "PASSIVE" | "PTT";

type AmbientMood = "calm" | "active" | "thinking" | "speaking";

// ─── Helpers ──────────────────────────────────────────────────────────────────

/** Map interaction state → ambient background mood */
function toMood(
  state: InteractionState,
  sleeping: boolean
): AmbientMood {
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

/** Human-readable, single label for the top-right status capsule */
function toStatusLabel(
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

/** Whether the status capsule dot should pulse */
function isDotActive(
  state: InteractionState,
  engaged: boolean,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  isPaused: boolean
): boolean {
  if (!engaged || isPaused) return false;
  return (
    state === "UserSpeaking" ||
    state === "Thinking" ||
    state === "AssistantSpeaking" ||
    ptt === "RECORDING" ||
    ptt === "PROCESSING"
  );
}

// ─── Test clips metadata ──────────────────────────────────────────────────────

const TEST_CLIPS = [
  { id: "short_en", name: "Quick English", duration: "~5s", desc: "Short English query" },
  { id: "short_hi", name: "Quick Hindi", duration: "~8s", desc: "Short Hindi query" },
  { id: "hinglish", name: "Hinglish Mix", duration: "~10s", desc: "Code-switching (EN+HI)" },
  { id: "command", name: "Command", duration: "~10s", desc: "Action-oriented command" },
  { id: "expressive", name: "Expressive", duration: "~16s", desc: "Longer, triggers emotion tags" },
] as const;


// ─── Component ────────────────────────────────────────────────────────────────

export const Home: React.FC = () => {
  const hasActiveTurnStarted = useRef(false);
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("PASSIVE");
  const [isEngaged, setIsEngaged] = useState(false);
  const [pipelineMode, setPipelineMode] = useState<"modular" | "realtime">("modular");
  const pipelineModeRef = useRef(pipelineMode);
  useEffect(() => {
    pipelineModeRef.current = pipelineMode;
  }, [pipelineMode]);
  const [isPaused, setIsPaused] = useState(false);
  const [sessionResumed, setSessionResumed] = useState(false);
  const [idleTimeout, setIdleTimeout] = useState<number | null>(null);
  const [hasCachedSession, setHasCachedSession] = useState(false);
  const [pttStatus, setPttStatus] = useState<"IDLE" | "RECORDING" | "PROCESSING">("IDLE");
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [isSleeping, setIsSleeping] = useState(false);
  const [cpuWarning, setCpuWarning] = useState<{ governor: string } | null>(null);
  const [isLaunching, setIsLaunching] = useState(false);
  const [testMode, setTestMode] = useState(false);
  const [testingClip, setTestingClip] = useState<string | null>(null);
  const testingClipRef = useRef<string | null>(null);
  const telemetryRef = useTelemetry();
  const [errorAlert, setErrorAlert] = useState<string | null>(null);
  const navigate = useNavigate();

  // Dialogue history system
  const [dialogueHistory, setDialogueHistory] = useState<{ user: string; assistant: string; id: number }[]>([]);
  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const turnIdCounter = useRef(0);

  // Layout refs
  const testButtonRef = useRef<HTMLButtonElement>(null);
  const testPanelRef = useRef<HTMLDivElement>(null);
  const dialogueScrollRef = useRef<HTMLDivElement>(null);
  const engageLockRef = useRef(false);
  const engageTimeoutRef = useRef<NodeJS.Timeout | number | null>(null);
  const prevEngagedRef = useRef(false);

  // Streaming text hooks
  const streamedTranscript = useStreamingRenderer(transcript);
  const streamedAssistantText = useStreamingRenderer(assistantText);

  // Mobile layout detection
  const [isMobileScreen, setIsMobileScreen] = useState(false);
  useEffect(() => {
    const checkMobile = () => setIsMobileScreen(window.innerWidth < 768);
    checkMobile();
    window.addEventListener("resize", checkMobile);
    return () => window.removeEventListener("resize", checkMobile);
  }, []);

  // Derived state
  const isThinking = interactionState === "Thinking" || pttStatus === "PROCESSING";
  const ambientMood = toMood(interactionState, isSleeping);
  const statusLabel = toStatusLabel(interactionState, isEngaged, isSleeping, pttStatus, isPaused, idleTimeout) + (sessionResumed && pipelineMode === "realtime" ? " (Resumed)" : "");
  const dotActive = isDotActive(interactionState, isEngaged, pttStatus, isPaused);

  // Keep testingClipRef in sync
  useEffect(() => {
    testingClipRef.current = testingClip;
  }, [testingClip]);

  // Archive current turn if there is any user or assistant content
  const archiveCurrentTurn = useCallback(() => {
    const userText = activeUserTextRef.current.trim();
    const aiText = activeAiTextRef.current.trim();
    if (userText || aiText) {
      turnIdCounter.current += 1;
      setDialogueHistory((prev) => {
        const next = [...prev, { user: userText, assistant: aiText, id: turnIdCounter.current }];
        // Only cap in modular mode — realtime sessions show full session history
        return pipelineModeRef.current === "modular" ? next.slice(-4) : next;
      });
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
      setTranscript("");
      setAssistantText("");
    }
  }, []);

  // Clear dialogue history and transcript refs when session ends (isEngaged becomes false)
  useEffect(() => {
    if (prevEngagedRef.current && !isEngaged) {
      setDialogueHistory([]);
      setTranscript("");
      setAssistantText("");
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
    }
    prevEngagedRef.current = isEngaged;
  }, [isEngaged]);

  // Auto-scroll to bottom of dialogue zone when transcript or history updates
  useEffect(() => {
    if (dialogueScrollRef.current) {
      dialogueScrollRef.current.scrollTo({
        top: dialogueScrollRef.current.scrollHeight,
        behavior: "auto",
      });
    }
  }, [dialogueHistory, streamedTranscript, streamedAssistantText]);

  // Click outside to close test clips panel
  useEffect(() => {
    if (!testMode) return;
    const handleClickOutside = (event: MouseEvent | TouchEvent) => {
      if (
        testButtonRef.current &&
        !testButtonRef.current.contains(event.target as Node) &&
        testPanelRef.current &&
        !testPanelRef.current.contains(event.target as Node)
      ) {
        setTestMode(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("touchstart", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("touchstart", handleClickOutside);
    };
  }, [testMode]);

  // Auto-reset test state when pipeline finishes (returns to Idle)
  useEffect(() => {
    if (interactionState === "Idle" && testingClip && hasActiveTurnStarted.current) {
      archiveCurrentTurn();
      setTestingClip(null);
      setIsEngaged(false);
      hasActiveTurnStarted.current = false;
    }
  }, [interactionState, testingClip, archiveCurrentTurn]);

  // ── Handlers ────────────────────────────────────────────────────────────────

  const handleCancelTest = async () => {
    try {
      await invoke("test_clip_cancel");
      archiveCurrentTurn();
      setTestingClip(null);
      setTranscript("");
      setAssistantText("");
      setIsEngaged(false);
      hasActiveTurnStarted.current = false;
    } catch (err) {
      console.error("[Home] Test clip cancel failed:", err);
    }
  };

  const handleEngage = useCallback(async () => {
    if (engageLockRef.current) return;
    engageLockRef.current = true;

    if (testingClip) {
      await handleCancelTest();
      engageLockRef.current = false;
      return;
    }
    archiveCurrentTurn();
    setIsLaunching(true);
    setErrorAlert(null);
    try {
      // Fetch latest settings to check pipeline mode
      const settings = await invoke<{ interaction?: { pipeline_mode?: string } }>("get_settings");
      const mode = settings?.interaction?.pipeline_mode || "modular";

      if (mode === "realtime") {
        await invoke("start_realtime_session");
        setIsEngaged(true);
        setIsPaused(false);
        setPipelineMode("realtime");
      } else {
        await invoke("engage");
        setIsEngaged(true);
        setIsPaused(false);
        setPipelineMode("modular");
      }
      setTranscript("");
      setAssistantText("");
    } catch (err) {
      console.error("[Home] Engagement failed:", err);
      const errMsg = typeof err === "string" ? err : (err instanceof Error ? err.message : String(err));
      setErrorAlert(errMsg);
    } finally {
      setIsLaunching(false);
      engageTimeoutRef.current = setTimeout(() => {
        engageLockRef.current = false;
      }, 800) as any;
    }
  }, [testingClip, archiveCurrentTurn]);

  const handleEnd = useCallback(async () => {
    if (!isEngaged || engageLockRef.current) return;
    engageLockRef.current = true;
    setIsLaunching(true);
    setErrorAlert(null);

    try {
      if (pipelineMode === "realtime") {
        await invoke("stop_realtime_session");
      } else {
        await invoke("engage"); // Toggle off
      }
      setIsEngaged(false);
      setIsPaused(false);
      setTranscript("");
      setAssistantText("");
      setDialogueHistory([]);
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
    } catch (err) {
      console.error("[Home] End session failed:", err);
    } finally {
      setIsLaunching(false);
      engageTimeoutRef.current = setTimeout(() => {
        engageLockRef.current = false;
      }, 800) as any;
    }
  }, [isEngaged, pipelineMode]);

  const handlePause = useCallback(async () => {
    if (!isEngaged || isPaused) return;
    try {
      await invoke("pause_pipeline");
      setIsPaused(true);
      archiveCurrentTurn();
    } catch (err) {
      console.error("[Home] Pause failed:", err);
    }
  }, [isEngaged, isPaused, archiveCurrentTurn]);

  const handleResume = useCallback(async () => {
    if (!isEngaged || !isPaused) return;
    try {
      await invoke("resume_pipeline");
      setIsPaused(false);
      setTranscript("");
      setAssistantText("");
    } catch (err) {
      console.error("[Home] Resume failed:", err);
    }
  }, [isEngaged, isPaused]);

  // ── Keyboard Keybindings ───────────────────────────────────────────────────
  useEffect(() => {
    const isInputActive = () => {
      const activeEl = document.activeElement;
      if (!activeEl) return false;
      const tagName = activeEl.tagName.toLowerCase();
      return (
        tagName === "input" ||
        tagName === "textarea" ||
        tagName === "select" ||
        activeEl.getAttribute("contenteditable") === "true"
      );
    };

    const isSpacePressedRef = { current: false };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (isInputActive()) return;
      if (e.repeat) return;

      const key = e.key.toLowerCase();

      if (e.code === "Space") {
        if (interactionMode === "PTT" && isEngaged && !isPaused) {
          e.preventDefault();
          if (!isSpacePressedRef.current) {
            isSpacePressedRef.current = true;
            archiveCurrentTurn();
            invoke("ptt_start", { owner: "MainWindow" }).catch((err) => {
              console.error("[Home] PTT start failed:", err);
              isSpacePressedRef.current = false;
            });
          }
        }
      } else if (key === "s") {
        e.preventDefault();
        if (isEngaged) {
          handleEnd();
        } else {
          handleEngage();
        }
      } else if (key === "p") {
        e.preventDefault();
        handlePause();
      } else if (key === "r") {
        e.preventDefault();
        handleResume();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space") {
        if (isSpacePressedRef.current) {
          isSpacePressedRef.current = false;
          if (interactionMode === "PTT" && isEngaged && !isPaused) {
            e.preventDefault();
            invoke("ptt_stop", { owner: "MainWindow" }).catch((err) => {
              console.error("[Home] PTT stop failed:", err);
            });
          }
        }
      }
    };

    const handleBlur = () => {
      if (isSpacePressedRef.current) {
        isSpacePressedRef.current = false;
        if (interactionMode === "PTT" && isEngaged && !isPaused) {
          invoke("ptt_stop", { owner: "MainWindow" }).catch((err) => {
            console.error("[Home] PTT stop on blur failed:", err);
          });
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleBlur);
      if (isSpacePressedRef.current && interactionMode === "PTT" && isEngaged && !isPaused) {
        invoke("ptt_stop", { owner: "MainWindow" }).catch(() => {});
      }
    };
  }, [isEngaged, isPaused, interactionMode, handleEngage, handleEnd, handlePause, handleResume, archiveCurrentTurn]);

  const togglePtt = async () => {
    if (!isEngaged || isPaused) return;
    try {
      if (pttStatus === "IDLE") {
        archiveCurrentTurn();
        await invoke("ptt_start", { owner: "MainWindow" });
      } else {
        await invoke("ptt_stop", { owner: "MainWindow" });
      }
    } catch (err) {
      console.error("[Home] PTT toggle failed:", err);
    }
  };

  const handleTestClip = async (clipId: string) => {
    if (isEngaged) return;
    archiveCurrentTurn();
    hasActiveTurnStarted.current = false;
    setTestingClip(clipId);
    setIsEngaged(true);
    setTestMode(false);
    setTranscript("");
    setAssistantText("");
    try {
      await invoke("test_clip", { clipId });
    } catch (err) {
      console.error("[Home] Test clip failed:", err);
      setTestingClip(null);
      setIsEngaged(false);
    }
  };

  // ── Session Cache Check ──────────────────────────────────────────────────────
  useEffect(() => {
    if (!isEngaged) {
      invoke<{ has_session: boolean }>("get_realtime_session_cache")
        .then((cache) => setHasCachedSession(cache.has_session))
        .catch((err) => console.warn("[Home] Failed to check session cache:", err));
    } else {
      setHasCachedSession(false);
    }
  }, [isEngaged]);

  // ── Tauri event listeners ────────────────────────────────────────────────────

  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        const appWindow = getCurrentWindow();

        const settings = await invoke<{ interaction?: { main_app_mode?: string; pipeline_mode?: string } }>("get_settings");
        if (settings?.interaction?.main_app_mode) {
          setInteractionMode(settings.interaction.main_app_mode.toUpperCase() as InteractionMode);
        }
        if (settings?.interaction?.pipeline_mode) {
          setPipelineMode(settings.interaction.pipeline_mode.toLowerCase() as "modular" | "realtime");
        }

        try {
          const snapshot = await invoke<{
            is_engaged?: boolean;
            is_sleeping?: boolean;
            cpu_governor?: string;
            cpu_governor_optimal?: boolean;
            conversation_id?: number;
          }>("get_runtime_snapshot");
          if (snapshot) {
            const engaged = snapshot.is_engaged ?? false;
            setIsEngaged(engaged);
            prevEngagedRef.current = engaged;
            setIsSleeping(snapshot.is_sleeping ?? false);
            if (snapshot.cpu_governor && !snapshot.cpu_governor_optimal) {
              setCpuWarning({ governor: snapshot.cpu_governor });
            }

            // Hydrate dialogue history if session is active
            if (engaged && snapshot.conversation_id && snapshot.conversation_id !== 0) {
              const turns = await invoke<{ user_text: string; assistant_text: string; turn_id: number }[]>("get_turns", {
                sessionId: snapshot.conversation_id,
              });
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
        } catch (e) {
          console.warn("[Home] Failed to sync initial state:", e);
        }

        unlisteners.push(
          await appWindow.listen<InteractionState>("state_changed", (event) => {
            const newState = event.payload;
            setInteractionState(newState);
            if (newState !== "Idle") {
              hasActiveTurnStarted.current = true;
              setIdleTimeout(null);
            }
            // Archive old turns when starting speech or resetting to Listening/Idle
            if (newState === "UserSpeaking" || newState === "Listening") {
              archiveCurrentTurn();
            }
          })
        );

        unlisteners.push(
          await appWindow.listen<{ text: string }>("transcript_partial", (event) => {
            setTranscript(event.payload.text);
            activeUserTextRef.current = event.payload.text;
            setIdleTimeout(null);
          })
        );

        unlisteners.push(
          await appWindow.listen<{ text: string }>("transcript_final", (event) => {
            setTranscript(event.payload.text);
            activeUserTextRef.current = event.payload.text;
            setIdleTimeout(null);
          })
        );

        unlisteners.push(
          await appWindow.listen<string>("llm_token", (event) => {
            setAssistantText(event.payload);
            activeAiTextRef.current = event.payload;
            setIdleTimeout(null);
          })
        );

        unlisteners.push(
          await appWindow.listen<string>("mode_changed_main", (event) => {
            setInteractionMode(event.payload.toUpperCase() as InteractionMode);
          })
        );

        unlisteners.push(
          await appWindow.listen<{ state: string }>("ptt_status", (event) => {
            setPttStatus(event.payload.state as "IDLE" | "RECORDING" | "PROCESSING");
            if (event.payload.state === "RECORDING") {
              archiveCurrentTurn();
              setAssistantText("");
              setTranscript("");
            }
          })
        );

        unlisteners.push(
          await appWindow.listen<boolean>("auto_sleep_state", (event) => {
            setIsSleeping(event.payload);
          })
        );

        unlisteners.push(
          await listen<{ governor: string; optimal: boolean }>("cpu_governor_warning", (event) => {
            if (!event.payload.optimal) {
              setCpuWarning({ governor: event.payload.governor });
            }
          })
        );

        unlisteners.push(
          await appWindow.listen("playback_finished", () => {
            if (testingClipRef.current) {
              archiveCurrentTurn();
              setTestingClip(null);
              setIsEngaged(false);
              hasActiveTurnStarted.current = false;
            }
          })
        );

        unlisteners.push(
          await appWindow.listen<string | undefined>("pipeline_error", (event) => {
            if (testingClipRef.current) {
              archiveCurrentTurn();
              setTestingClip(null);
              setIsEngaged(false);
              hasActiveTurnStarted.current = false;
            }
            if (pipelineModeRef.current === "realtime") {
              setIsPaused(true);
              console.error("[Home] Realtime S2S connection error:", event.payload);
              setErrorAlert(event.payload || "Realtime connection to Gemini Live lost.");
            }
          })
        );

        unlisteners.push(
          await appWindow.listen("pipeline_paused", () => {
            setIsPaused(true);
            archiveCurrentTurn();
          })
        );

        unlisteners.push(
          await appWindow.listen("pipeline_resumed", () => {
            setIsPaused(false);
          })
        );

        unlisteners.push(
          await appWindow.listen("realtime_session_started", () => {
            setPipelineMode("realtime");
            setSessionResumed(false);
          })
        );

        unlisteners.push(
          await appWindow.listen("realtime_session_resumed", () => {
            setPipelineMode("realtime");
            setSessionResumed(true);
          })
        );

        unlisteners.push(
          await appWindow.listen<string>("realtime_session_ended", (event) => {
            const reason = event.payload; // "user", "idle_timeout", "error"
            setIsEngaged(false);
            setIsPaused(false);
            setDialogueHistory([]);
            setTranscript("");
            setAssistantText("");
            activeUserTextRef.current = "";
            activeAiTextRef.current = "";
            console.log("[Home] Realtime S2S session ended. Reason:", reason);
          })
        );

        unlisteners.push(
          await appWindow.listen("realtime_interrupted", () => {
            setInteractionState("Interrupted");
            setTimeout(() => setInteractionState("UserSpeaking"), 150);
          })
        );

        unlisteners.push(
          await appWindow.listen<{ seconds_remaining: number }>("realtime_idle_warning", (event) => {
            setIdleTimeout(event.payload.seconds_remaining);
          })
        );

        setTimeout(async () => {
          await invoke("show_main_window");
        }, 300);
      } catch (err) {
        console.error("[Home] Failed to setup Tauri listeners:", err);
      }
    };

    setup();
    return () => {
      if (engageTimeoutRef.current) {
        clearTimeout(engageTimeoutRef.current as any);
      }
      unlisteners.forEach((u) => u());
    };
  }, []);

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Reactive ambient background — responds to interaction state */}
      <AmbientBackground mood={ambientMood} originX="50%" originY="47%" />

      {/* Sentient Field Background Energy */}
      <PipelineField state={interactionState} />

      {/* Floating Error Toast */}
      <AnimatePresence>
        {errorAlert && (
          <motion.div
            initial={{ opacity: 0, x: 50, scale: 0.95 }}
            animate={{ opacity: 1, x: 0, scale: 1 }}
            exit={{ opacity: 0, x: 50, scale: 0.95 }}
            className="absolute top-4 left-4 right-4 md:left-auto md:right-4 z-[100] md:max-w-sm pointer-events-auto"
          >
            <div className="glass-card p-4 rounded-xl flex items-start gap-3 border border-red-500/30 shadow-2xl bg-black/40 backdrop-blur-md">
              <AlertCircle className="text-red-400 shrink-0 mt-0.5" size={18} />
              <div className="flex-1 flex flex-col gap-1.5 min-w-0">
                <span className="text-xs font-bold tracking-wider uppercase text-red-400 text-left">Connection Error</span>
                <p className="text-[11px] text-[rgb(var(--foreground))]/90 leading-relaxed font-light break-words select-text text-left">
                  {errorAlert}
                </p>
                <div className="flex gap-3 mt-1 justify-start">
                  <button
                    onClick={() => {
                      setErrorAlert(null);
                      navigate("/settings");
                    }}
                    className="text-[10px] font-black uppercase tracking-wider text-[rgb(var(--accent))] hover:underline cursor-pointer"
                  >
                    Configure Settings
                  </button>
                  <button
                    onClick={() => setErrorAlert(null)}
                    className="text-[10px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer"
                  >
                    Dismiss
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Top-right: Status Capsule (single, clean, centered on mobile) ── */}
      <div className="absolute top-[10%] md:top-4 left-1/2 -translate-x-1/2 md:left-auto md:translate-x-0 md:right-5 z-30 flex items-center gap-2 pointer-events-none">
        {cpuWarning && (
          <span className="text-[9px] font-mono tracking-widest uppercase text-[rgb(var(--accent))]/60">
            CPU: {cpuWarning.governor}
          </span>
        )}
        <StatusCapsule
          label={statusLabel}
          dotActive={dotActive}
          testing={!!testingClip}
        />
      </div>

      {/* ── Side Dialogue Area - Right Only (All transcripts, big screens only) ── */}
      <div
        className="absolute top-[64px] bottom-[20%] right-0 flex flex-col justify-end items-center pointer-events-none hidden md:flex z-20"
        style={{ width: "calc(50vw - min(35vw, 32.5vh))" }}
      >
        <div
          ref={dialogueScrollRef}
          className="w-full max-h-[85%] overflow-y-auto scrollbar-none flex flex-col items-center gap-6 pointer-events-auto select-text px-4 pb-6"
          style={{
            maskImage: "linear-gradient(to bottom, transparent 0%, black 15%, black 85%, transparent 100%)",
            WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 15%, black 85%, transparent 100%)",
          }}
        >
          <div className="flex-1 min-h-[4vh]" />
          {/* Dialogue History */}
          {dialogueHistory.map((turn) => (
            <React.Fragment key={turn.id}>
              {turn.user && (
                <div className="w-full max-w-[220px] break-words text-left text-[rgb(var(--foreground))]/65 font-light text-[13px] leading-relaxed opacity-90 prose prose-invert select-text">
                  <span className="text-[11px] font-mono tracking-widest text-[rgb(var(--foreground-muted))] uppercase block mb-0.5">
                    [USER]
                  </span>
                  <ReactMarkdown components={MarkdownComponents}>{turn.user}</ReactMarkdown>
                </div>
              )}
              {turn.assistant && (
                <div className="w-full max-w-[220px] break-words text-left text-[rgb(var(--accent))] font-medium text-[13px] leading-relaxed opacity-90 prose prose-invert select-text" style={{ textShadow: "0 0 15px rgba(var(--accent), 0.15)" }}>
                  <span className="text-[11px] font-mono tracking-widest text-[rgb(var(--accent))]/90 uppercase block mb-0.5">
                    [VOX]
                  </span>
                  <ReactMarkdown components={MarkdownComponents}>{turn.assistant}</ReactMarkdown>
                </div>
              )}
            </React.Fragment>
          ))}

          {/* Active Current Turn */}
          {(streamedTranscript || streamedAssistantText) && (
            <div className="w-full flex flex-col gap-6 items-center">
              {streamedTranscript && (
                <motion.div
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  className="w-full max-w-[220px] break-words text-left text-[rgb(var(--foreground))]/85 font-light text-[11px] leading-relaxed prose prose-invert select-text"
                >
                  <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--foreground-muted))]/60 uppercase block mb-0.5">
                    [USER]
                  </span>
                  <ReactMarkdown components={MarkdownComponents}>{streamedTranscript}</ReactMarkdown>
                </motion.div>
              )}
              {streamedAssistantText && (
                <motion.div
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  className="w-full max-w-[220px] break-words text-left text-[rgb(var(--accent))] font-medium text-[11px] leading-relaxed prose prose-invert select-text" style={{ textShadow: "0 0 25px rgba(var(--accent), 0.25)" }}
                >
                  <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--accent))]/70 uppercase block mb-0.5">
                    [VOX]
                  </span>
                  <ReactMarkdown components={MarkdownComponents}>{streamedAssistantText}</ReactMarkdown>
                </motion.div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* ── Orb Stage (center of lower half) ───────────────────────────── */}
      <div
        className="absolute z-10 pointer-events-none overflow-hidden flex items-center justify-center"
        style={{
          left: "50%",
          top: "47%",
          transform: "translate(-50%, -50%)",
          width: isMobileScreen ? "min(92vw, 85vh)" : "min(70vw, 65vh)",
          height: isMobileScreen ? "min(92vw, 85vh)" : "min(70vw, 65vh)",
          minWidth: 280,
          minHeight: 280,
          maxWidth: 660,
          maxHeight: 660,
        }}
      >
        {/* Subtle dynamic ring behind orb */}
        <div
          className={cn(
            "absolute inset-0 rounded-full border border-[rgb(var(--accent))]/10 transition-all duration-1000",
            isEngaged ? "scale-100 opacity-100 animate-field-pulse" : "scale-90 opacity-60"
          )}
        />
        <div className="relative w-full h-full flex items-center justify-center">
          <ErrorBoundary name="VoxOrb">
            <VoxOrb
              telemetryRef={telemetryRef}
              interactionState={interactionState}
              isSleeping={isSleeping}
              isTesting={!!testingClip}
            />
          </ErrorBoundary>
        </div>
      </div>

      {/* ── Bottom Controls ─────────────────────────────────────────────── */}
      <div className="absolute bottom-[10%] left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-4 w-full max-w-md">
        {/* Buttons */}
        <div className="flex items-center gap-4 relative">
          {/* Universal Pause / Resume Button */}
          {isEngaged && !testingClip && (
            <button
              onClick={isPaused ? handleResume : handlePause}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95",
                isPaused
                  ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--accent))]"
              )}
              aria-label={isPaused ? "Resume Vox" : "Pause Vox"}
            >
              {isPaused ? <Play size={28} /> : <Pause size={28} />}
            </button>
          )}

          {/* PTT Mic Button */}
          {isEngaged && !testingClip && interactionMode === "PTT" && (
            <button
              onClick={togglePtt}
              disabled={isPaused}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95",
                pttStatus === "RECORDING"
                  ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--accent))]",
                isPaused && "opacity-40 cursor-not-allowed hover:bg-transparent hover:scale-100"
              )}
              aria-label="Toggle PTT Microphone"
            >
              <Mic size={28} className={cn(pttStatus === "RECORDING" && "animate-pulse-slow")} />
            </button>
          )}

          {/* Primary Engage / Disengage Button */}
          <div className="relative flex flex-col items-center">
            {!isEngaged && hasCachedSession && (
              <span className="absolute -top-7 text-[9px] font-mono tracking-widest text-[rgb(var(--accent))]/85 uppercase animate-pulse whitespace-nowrap bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded-full border border-[rgb(var(--accent))]/15">
                Resume Session
              </span>
            )}
            <button
              onClick={isEngaged ? handleEnd : handleEngage}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95",
                isEngaged && isThinking && "engage-btn-loading border-transparent",
                isLaunching && "animate-spin",
                isEngaged
                  ? "border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/15"
                  : "bg-transparent text-[rgb(var(--accent))]"
              )}
              disabled={isLaunching}
              aria-label={isEngaged ? "Stop Vox" : (hasCachedSession ? "Resume Vox Session" : "Engage Vox")}
            >
              {isLaunching ? (
                <Power size={28} className="animate-pulse-slow" />
              ) : isEngaged ? (
                <X size={28} />
              ) : (
                <Power
                  size={28}
                  className="transition-transform duration-700"
                />
              )}
            </button>
          </div>
        </div>
      </div>

      {/* ── Test Mode — bottom-right, hidden when engaged ──────────────── */}
      <AnimatePresence>
        {!isEngaged && (
          <motion.div
            key="test-mode-container"
            initial={{ opacity: 0, scale: 0.85, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.85, y: 10 }}
            transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
            className="hidden md:block fixed bottom-4 right-4 z-50"
          >
            <button
              ref={testButtonRef}
              onClick={() => setTestMode(!testMode)}
              className={cn(
                "flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 cursor-pointer glass-card",
                testMode
                  ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/60"
                  : "bg-transparent border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
              )}
              aria-label="Test Mode"
            >
              <FlaskConical size={22} />
            </button>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Test Mode Panel ──────────────── */}
      <AnimatePresence>
        {testMode && !isEngaged && (
          <motion.div
            key="test-mode-panel"
            ref={testPanelRef}
            initial={{ opacity: 0, y: 8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1], opacity: { duration: 0.1 } }}
            className="fixed bottom-16 right-4 w-56 p-2 flex flex-col gap-1 z-50 glass-card no-blur"
          >
            <div className="px-2 py-1 border-b border-[rgba(var(--accent),0.1)] mb-1">
              <span className="text-[10px] font-mono tracking-widest text-[rgb(var(--accent))] uppercase block">
                Select Test Input
              </span>
            </div>
            {TEST_CLIPS.map((clip) => (
              <button
                key={clip.id}
                onClick={() => handleTestClip(clip.id)}
                className="w-full text-left p-2 rounded-xl hover:bg-[rgb(var(--accent))]/10 transition-colors border border-transparent hover:border-[rgb(var(--accent))]/15 flex flex-col"
              >
                <span className="text-[13px] font-semibold text-[rgb(var(--foreground))]">
                  {clip.name}
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
                  {clip.desc} · {clip.duration}
                </span>
              </button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>

    </div>
  );
};
