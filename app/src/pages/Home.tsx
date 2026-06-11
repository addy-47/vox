import React, { useState, useEffect, useRef , useCallback} from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { ErrorBoundary } from "@/shared/components/ErrorBoundary";
import { LiveWaveform } from "@/shared/components/LiveWaveform";
import { PipelineField } from "@/shared/components/PipelineField";
import { AmbientBackground } from "@/shared/components/AmbientBackground";
import { StatusCapsule } from "@/shared/components/StatusCapsule";
import { useStreamingRenderer } from "@/shared/hooks/useStreamingRenderer";
import { Power, Mic, FlaskConical } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";

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
  ptt: "IDLE" | "RECORDING" | "PROCESSING"
): string {
  if (!engaged) return "Dormant";
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
  ptt: "IDLE" | "RECORDING" | "PROCESSING"
): boolean {
  if (!engaged) return false;
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
  const isUserSpeaking = interactionState === "UserSpeaking" || pttStatus === "RECORDING";
  const isThinking = interactionState === "Thinking" || pttStatus === "PROCESSING";
  const activeSpeaking = isUserSpeaking;
  const ambientMood = toMood(interactionState, isSleeping);
  const statusLabel = toStatusLabel(interactionState, isEngaged, isSleeping, pttStatus);
  const dotActive = isDotActive(interactionState, isEngaged, pttStatus);

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
        return next.slice(-4); // Keep last 4 turns for contextual flow
      });
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
    }
  }, []);

  // Clear dialogue history and transcript refs when session ends (isEngaged becomes false)
  useEffect(() => {
    if (!isEngaged) {
      setDialogueHistory([]);
      setTranscript("");
      setAssistantText("");
      activeUserTextRef.current = "";
      activeAiTextRef.current = "";
    }
  }, [isEngaged]);

  // Auto-scroll to bottom of dialogue zone when transcript or history updates
  useEffect(() => {
    if (dialogueScrollRef.current) {
      dialogueScrollRef.current.scrollTo({
        top: dialogueScrollRef.current.scrollHeight,
        behavior: "smooth",
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
      hasActiveTurnStarted.current = false;
    } catch (err) {
      console.error("[Home] Test clip cancel failed:", err);
    }
  };

  const handleEngage = async () => {
    if (engageLockRef.current) return;
    engageLockRef.current = true;

    if (testingClip) {
      await handleCancelTest();
      engageLockRef.current = false;
      return;
    }
    archiveCurrentTurn();
    setIsLaunching(true);
    try {
      await invoke("engage");
      const newEngaged = !isEngaged;
      setIsEngaged(newEngaged);
      setTranscript("");
      setAssistantText("");
    } catch (err) {
      console.error("[Home] Engagement failed:", err);
    } finally {
      setIsLaunching(false);
      setTimeout(() => {
        engageLockRef.current = false;
      }, 800);
    }
  };

  const togglePtt = async () => {
    if (!isEngaged) return;
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

  // ── Tauri event listeners ────────────────────────────────────────────────────

  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        const appWindow = getCurrentWindow();

        const settings = await invoke<{ main_app_mode?: string }>("get_settings");
        if (settings?.main_app_mode) {
          setInteractionMode(settings.main_app_mode.toUpperCase() as InteractionMode);
        }

        try {
          const snapshot = await invoke<{
            is_engaged?: boolean;
            is_sleeping?: boolean;
            cpu_governor?: string;
            cpu_governor_optimal?: boolean;
          }>("get_runtime_snapshot");
          if (snapshot) {
            setIsEngaged(snapshot.is_engaged ?? false);
            setIsSleeping(snapshot.is_sleeping ?? false);
            if (snapshot.cpu_governor && !snapshot.cpu_governor_optimal) {
              setCpuWarning({ governor: snapshot.cpu_governor });
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
          })
        );

        unlisteners.push(
          await appWindow.listen<{ text: string }>("transcript_final", (event) => {
            setTranscript(event.payload.text);
            activeUserTextRef.current = event.payload.text;
          })
        );

        unlisteners.push(
          await appWindow.listen<string>("llm_token", (event) => {
            setAssistantText(event.payload);
            activeAiTextRef.current = event.payload;
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
          await appWindow.listen("pipeline_error", () => {
            if (testingClipRef.current) {
              archiveCurrentTurn();
              setTestingClip(null);
              setIsEngaged(false);
              hasActiveTurnStarted.current = false;
            }
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
    return () => unlisteners.forEach((u) => u());
  }, [archiveCurrentTurn]);

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Reactive ambient background — responds to interaction state */}
      <AmbientBackground mood={ambientMood} originX="50%" originY="55%" />

      {/* Sentient Field Background Energy */}
      <PipelineField state={interactionState} />

      {/* ── Top-right: Status Capsule (single, clean) ──────────────────── */}
      <div className="absolute top-4 right-5 z-30 flex items-center gap-2 pointer-events-none">
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
                <div className="w-full max-w-[150px] break-words text-left text-[rgb(var(--foreground))]/65 font-light text-[13px] leading-relaxed">
                  <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--foreground-muted))]/40 uppercase block mb-0.5">
                    [USER]
                  </span>
                  {turn.user}
                </div>
              )}
              {turn.assistant && (
                <div className="w-full max-w-[150px] break-words text-left text-[rgb(var(--accent))]/80 font-medium text-[13px] leading-relaxed" style={{ textShadow: "0 0 15px rgba(var(--accent), 0.15)" }}>
                  <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--accent))]/50 uppercase block mb-0.5">
                    [VOX]
                  </span>
                  {turn.assistant}
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
                  className="w-full max-w-[150px] break-words text-left text-[rgb(var(--foreground))]/85 font-light text-[13px] leading-relaxed"
                >
                  <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--foreground-muted))]/60 uppercase block mb-0.5">
                    [USER]
                  </span>
                  {streamedTranscript}
                </motion.div>
              )}
              {streamedAssistantText && (
                <motion.div
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  className="w-full max-w-[150px] break-words text-left text-[rgb(var(--accent))] font-medium text-[13px] leading-relaxed" style={{ textShadow: "0 0 25px rgba(var(--accent), 0.25)" }}
                >
                  <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--accent))]/70 uppercase block mb-0.5">
                    [VOX]
                  </span>
                  {streamedAssistantText}
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
              interactionState={interactionState}
              isSleeping={isSleeping}
              isTesting={!!testingClip}
            />
          </ErrorBoundary>
        </div>
      </div>

      {/* ── Bottom Controls ─────────────────────────────────────────────── */}
      <div className="absolute bottom-[10%] left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-4 w-full max-w-md">
        {/* Waveform */}
        <div
          className={cn(
            "w-full h-10 transition-all duration-500 pointer-events-none opacity-0 scale-95",
            activeSpeaking && !testingClip && "opacity-75 scale-100"
          )}
        >
          <LiveWaveform
            active={activeSpeaking && !testingClip}
            processing={false}
            telemetryRef={telemetryRef}
            height={36}
            className="w-full"
            mode="static"
            barWidth={2.5}
            barGap={1.5}
            fadeWidth={80}
          />
        </div>

        {/* Buttons */}
        <div className="flex items-center gap-4 relative">
          {/* PTT Mic Button */}
          {isEngaged && !testingClip && interactionMode === "PTT" && (
            <button
              onClick={togglePtt}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/30 bg-black/35 hover:scale-105 active:scale-95",
                pttStatus === "RECORDING"
                  ? "bg-[rgb(var(--accent))] border-transparent text-[rgb(var(--accent-foreground))] shadow-[0_0_20px_rgba(var(--accent),0.4)]"
                  : "text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
              )}
              aria-label="Toggle PTT Microphone"
            >
              <Mic size={22} className={cn(pttStatus === "RECORDING" && "animate-pulse-slow")} />
            </button>
          )}

          {/* Primary Engage Button */}
          <div className="relative">
            <button
              onClick={handleEngage}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/30 bg-black/35 hover:scale-105 active:scale-95 shadow-[0_4px_20px_rgba(0,0,0,0.4)]",
                isEngaged && isThinking && "engage-btn-loading border-transparent",
                isLaunching && "animate-spin",
                isEngaged
                  ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.2)] bg-black/45"
                  : "bg-[rgb(var(--accent))]/10 hover:bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]"
              )}
              disabled={isLaunching}
              aria-label={isEngaged ? "Stop Vox" : "Engage Vox"}
            >
              {isLaunching ? (
                <Power size={20} className="animate-pulse-slow" />
              ) : (
                <Power
                  size={20}
                  className={cn("transition-transform duration-700", isEngaged && "rotate-180")}
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
            initial={{ opacity: 0, scale: 0.85, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.85, y: 10 }}
            transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
            className="hidden md:block fixed bottom-4 right-4 z-50"
          >
            <div className="relative">
              <button
                ref={testButtonRef}
                onClick={() => setTestMode(!testMode)}
                className={cn(
                  "flex items-center justify-center w-11 h-11 rounded-full border border-[rgba(var(--accent),0.15)] bg-black/45 hover:bg-[rgb(var(--accent))]/10 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all duration-300",
                  testMode && "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                )}
                aria-label="Test Mode"
              >
                <FlaskConical size={16} />
              </button>

              <AnimatePresence>
                {testMode && (
                  <motion.div
                    ref={testPanelRef}
                    initial={{ opacity: 0, y: 8, scale: 0.96 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: 8, scale: 0.96 }}
                    transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
                    className="absolute bottom-14 right-0 w-56 p-2 rounded-2xl bg-black/90 border border-[rgba(var(--accent),0.25)] shadow-[0_10px_30px_rgba(0,0,0,0.6)] backdrop-blur-xl flex flex-col gap-1 z-30"
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
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
