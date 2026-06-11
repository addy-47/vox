import React, { useState, useEffect, useRef } from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { ErrorBoundary } from "@/shared/components/ErrorBoundary";
import { LiveWaveform } from "@/shared/components/LiveWaveform";
import { PipelineField } from "@/shared/components/PipelineField";
import { AmbientBackground } from "@/shared/components/AmbientBackground";
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

// ─── Status Capsule ───────────────────────────────────────────────────────────

interface StatusCapsuleProps {
  label: string;
  dotActive: boolean;
  testing?: boolean;
}

const StatusCapsule: React.FC<StatusCapsuleProps> = ({ label, dotActive, testing }) => (
  <div className="flex items-center gap-2 px-3 py-1.5 rounded-full glass-elevated glass-base border border-[rgba(var(--accent),0.15)]">
    {testing ? (
      <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
    ) : (
      <span
        className={cn(
          "w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] transition-all duration-700",
          dotActive ? "opacity-100 shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "opacity-30"
        )}
        style={dotActive ? { animation: "pulse-slow 2.5s ease-in-out infinite" } : {}}
      />
    )}
    <span className="text-[10px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--foreground-muted))]">
      {testing ? "Testing" : label}
    </span>
  </div>
);

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

  // Auto-reset test state when pipeline finishes (returns to Idle)
  useEffect(() => {
    if (interactionState === "Idle" && testingClip && hasActiveTurnStarted.current) {
      setTestingClip(null);
      setIsEngaged(false);
      hasActiveTurnStarted.current = false;
    }
  }, [interactionState, testingClip]);

  // ── Handlers ────────────────────────────────────────────────────────────────

  const handleCancelTest = async () => {
    try {
      await invoke("test_clip_cancel");
      setTestingClip(null);
      setTranscript("");
      setAssistantText("");
      hasActiveTurnStarted.current = false;
    } catch (err) {
      console.error("[Home] Test clip cancel failed:", err);
    }
  };

  const handleEngage = async () => {
    if (testingClip) {
      await handleCancelTest();
      return;
    }
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
    }
  };

  const togglePtt = async () => {
    if (!isEngaged) return;
    try {
      if (pttStatus === "IDLE") {
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
            if (newState !== "Idle") hasActiveTurnStarted.current = true;
          })
        );

        unlisteners.push(
          await appWindow.listen<{ text: string }>("transcript_partial", (event) => {
            setTranscript(event.payload.text);
          })
        );

        unlisteners.push(
          await appWindow.listen<{ text: string }>("transcript_final", (event) => {
            setTranscript(event.payload.text);
          })
        );

        unlisteners.push(
          await appWindow.listen<string>("llm_token", (event) => {
            setAssistantText(event.payload);
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
              setTestingClip(null);
              setIsEngaged(false);
              hasActiveTurnStarted.current = false;
            }
          })
        );

        unlisteners.push(
          await appWindow.listen("pipeline_error", () => {
            if (testingClipRef.current) {
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
  }, []);

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

      {/* ── Dialogue Area (upper center — text rises from orb) ─────────── */}
      <div className="absolute top-0 left-0 right-0 bottom-[45%] flex flex-col items-center justify-end pb-8 px-8 z-20 pointer-events-none select-text">
        <AnimatePresence mode="wait">
          {transcript && (
            <motion.p
              key="transcript"
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
              className="field-text text-center text-[rgb(var(--foreground))]/65 font-light max-w-xl"
            >
              {transcript}
            </motion.p>
          )}
          {assistantText && !transcript && (
            <motion.p
              key="assistant"
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
              className="field-text text-center text-[rgb(var(--accent))] font-medium max-w-xl"
              style={{ textShadow: "0 0 30px rgba(var(--accent), 0.3)" }}
            >
              {assistantText}
            </motion.p>
          )}
          {assistantText && transcript && (
            <motion.p
              key="both"
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
              className="field-text text-center text-[rgb(var(--accent))] font-medium max-w-xl"
              style={{ textShadow: "0 0 30px rgba(var(--accent), 0.3)" }}
            >
              {assistantText}
            </motion.p>
          )}
        </AnimatePresence>
      </div>

      {/* ── Orb Stage (center of lower half) ───────────────────────────── */}
      <div
        className="absolute left-1/2 -translate-x-1/2 z-10 pointer-events-none overflow-hidden flex items-center justify-center"
        style={{
          top: "55%",
          transform: "translate(-50%, -50%)",
          width: "min(70vw, 65vh)",
          height: "min(70vw, 65vh)",
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
      <div className="absolute bottom-[2%] left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-4 w-full max-w-md">
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
      {!isEngaged && (
        <div className="hidden md:block fixed bottom-4 right-4 z-50">
          <div className="relative">
            <button
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
        </div>
      )}
    </div>
  );
};
