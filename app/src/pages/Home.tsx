import React, { useState, useEffect, useRef } from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { ErrorBoundary } from "@/shared/components/ErrorBoundary";
import { LiveWaveform } from "@/shared/components/LiveWaveform";
import { Activity, Mic, FlaskConical } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";
type InteractionMode = "PASSIVE" | "PTT";

export const Home: React.FC = () => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const hasActiveTurnStarted = useRef(false);
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("PASSIVE");
  const [isEngaged, setIsEngaged] = useState(false);
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [isSleeping, setIsSleeping] = useState(false);
  const [cpuWarning, setCpuWarning] = useState<{governor: string; advice: string} | null>(null);
  const telemetryRef = useTelemetry();

  const isUserSpeaking = interactionState === "UserSpeaking" || pttStatus === 'RECORDING';
  const isThinking = interactionState === "Thinking" || pttStatus === 'PROCESSING';

  // Waveform only reflects user speech activity.
  // Fades out during Thinking/Processing/AssistantSpeaking.
  const activeSpeaking = isUserSpeaking;

  const [isLaunching, setIsLaunching] = useState(false);
  const [testMode, setTestMode] = useState(false);
  const [testingClip, setTestingClip] = useState<string | null>(null);
  const testingClipRef = useRef<string | null>(null);

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

  // Auto-scroll to bottom of dialogue container
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [transcript, assistantText]);

  const testClips = [
    { id: "short_en", name: "Quick English", duration: "~5s", desc: "Short English query" },
    { id: "short_hi", name: "Quick Hindi", duration: "~8s", desc: "Short Hindi query" },
    { id: "hinglish", name: "Hinglish Mix", duration: "~10s", desc: "Code-switching (EN+HI)" },
    { id: "command", name: "Command", duration: "~10s", desc: "Action-oriented command" },
    { id: "expressive", name: "Expressive", duration: "~16s", desc: "Longer, triggers emotion tags" },
  ];

  const handleCancelTest = async () => {
    try {
      await invoke("test_clip_cancel");
      setTestingClip(null);
      setTranscript("");
      setAssistantText("");
      hasActiveTurnStarted.current = false;
      console.log("[Home] Test clip cancelled.");
    } catch (err) {
      console.error("[Home] Test clip cancel failed:", err);
    }
  };

  const handleEngage = async () => {
    // If a test clip is running, cancel it
    if (testingClip) {
      await handleCancelTest();
      setIsEngaged(false);
      return;
    }

    setIsLaunching(true);
    try {
      await invoke("engage");
      const newEngaged = !isEngaged;
      setIsEngaged(newEngaged);
      if (newEngaged) {
        setTranscript("");
        setAssistantText("");
      } else {
        // Clear dialogue on session stop so glass card resets
        setTranscript("");
        setAssistantText("");
      }
      console.log(newEngaged ? "[Home] Pipeline engaged." : "[Home] Pipeline disengaged.");
    } catch (err) {
      console.error("[Home] Engagement failed:", err);
    } finally {
      setIsLaunching(false);
    }
  };

  const togglePtt = async () => {
    if (!isEngaged) return;
    try {
      if (pttStatus === 'IDLE') {
        await invoke("ptt_start", { owner: "MainWindow" });
      } else {
        await invoke("ptt_stop", { owner: "MainWindow" });
      }
    } catch (err) {
      console.error("[Home] PTT toggle failed:", err);
    }
  };

  const handleTestClip = async (clipId: string) => {
    // Block if already live-engaged
    if (isEngaged) return;

    hasActiveTurnStarted.current = false;
    setTestingClip(clipId);
    setIsEngaged(true);
    setTestMode(false); // Collapse the dropdown
    setTranscript("");
    setAssistantText("");
    try {
      await invoke("test_clip", { clipId });
      console.log("[Home] Test clip sent:", clipId);
    } catch (err) {
      console.error("[Home] Test clip failed:", err);
      setTestingClip(null);
      setIsEngaged(false);
    }
  };

  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        const appWindow = getCurrentWindow();
        
        // Initial Settings - Fix: use main_app_mode
        const settings = await invoke<any>("get_settings");
        if (settings?.main_app_mode) {
          setInteractionMode(settings.main_app_mode.toUpperCase() as InteractionMode);
        }

        // Sync Engagement State
        try {
          const snapshot = await invoke<any>("get_runtime_snapshot");
          if (snapshot) {
            setIsEngaged(snapshot.is_engaged);
            setIsSleeping(snapshot.is_sleeping);
            // Check CPU governor from snapshot (reliable — no race condition)
            if (snapshot.cpu_governor && !snapshot.cpu_governor_optimal) {
              setCpuWarning({
                governor: snapshot.cpu_governor,
                advice: "Switch to 'performance' governor for best voice pipeline performance",
              });
            }
          }
        } catch (e) {
          console.warn("[Home] Failed to sync initial engagement state:", e);
        }

        unlisteners.push(await appWindow.listen<InteractionState>("state_changed", (event) => {
          const newState = event.payload;
          setInteractionState(newState);
          if (newState !== "Idle") {
            hasActiveTurnStarted.current = true;
          }
        }));

        unlisteners.push(await appWindow.listen<{text: string}>("transcript_partial", (event) => {
          setTranscript(event.payload.text);
        }));

        unlisteners.push(await appWindow.listen<{text: string}>("transcript_final", (event) => {
          setTranscript(event.payload.text);
        }));

        unlisteners.push(await appWindow.listen<string>("llm_token", (event) => {
          setAssistantText(event.payload);
        }));

        unlisteners.push(await appWindow.listen<string>("mode_changed_main", (event) => {
          setInteractionMode(event.payload.toUpperCase() as InteractionMode);
        }));

        unlisteners.push(await appWindow.listen<{ state: string }>("ptt_status", (event) => {
          setPttStatus(event.payload.state as any);
          if (event.payload.state === 'RECORDING') {
            setAssistantText(""); // Clear previous on new turn
            setTranscript("");
          }
        }));

        unlisteners.push(await appWindow.listen<boolean>("auto_sleep_state", (event) => {
          setIsSleeping(event.payload);
        }));

        // CPU Governor Warning (Linux only)
        unlisteners.push(await listen<{governor: string; optimal: boolean; advice: string}>("cpu_governor_warning", (event) => {
          if (!event.payload.optimal) {
            setCpuWarning({ governor: event.payload.governor, advice: event.payload.advice });
          }
        }));

        unlisteners.push(await appWindow.listen<any>("playback_finished", (_event) => {
          if (testingClipRef.current) {
            setTestingClip(null);
            setIsEngaged(false);
            hasActiveTurnStarted.current = false;
          }
        }));

        unlisteners.push(await appWindow.listen<any>("pipeline_error", (_event) => {
          if (testingClipRef.current) {
            setTestingClip(null);
            setIsEngaged(false);
            hasActiveTurnStarted.current = false;
          }
        }));

        // Phase 5: Show window only after listeners are ready
        setTimeout(async () => {
          await invoke("show_main_window");
        }, 300);
      } catch (err) {
        console.error("[Home] Failed to setup Tauri listeners:", err);
      }
    };

    setup();
    return () => {
      unlisteners.forEach(u => u());
    };
  }, []);


  return (
    <div className="flex-1 flex h-full w-full overflow-hidden bg-[rgb(var(--background))] transition-all duration-400 ease-in-out">
      {/* ===== CENTRAL HUD AREA ===== */}
      <div className="flex-1 flex flex-col relative overflow-visible">

        {/* CPU Governor Warning Banner */}
        {cpuWarning && (
          <div className="mx-6 mt-3 px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20 flex items-center justify-between gap-3 animate-in fade-in slide-in-from-top-2 duration-500">
            <div className="flex items-center gap-2.5">
              <span className="text-amber-400 text-[13px] font-bold shrink-0">⚠</span>
              <span className="text-[12px] text-amber-300/90 leading-snug">
                CPU governor is <span className="font-bold text-amber-200">{cpuWarning.governor}</span> — this may slow voice responses significantly.
                <br />
                <span className="opacity-70">{cpuWarning.advice}</span>
              </span>
            </div>
            <button
              onClick={() => setCpuWarning(null)}
              className="text-amber-400/60 hover:text-amber-300 text-[15px] font-bold shrink-0 px-1 transition-colors"
              title="Dismiss"
            >✕</button>
          </div>
        )}

        {/* Status Area */}
        <div className="p-6 md:p-12 pb-0 flex flex-col items-center gap-4 shrink-0">
          <div className="premium-card flex items-center gap-3 px-6 md:px-10 py-2.5 md:py-3">
            {testingClip && (
              <FlaskConical size={14} className="text-[rgb(var(--accent))] animate-pulse shrink-0" />
            )}
            <div className={cn(
              "w-2 md:w-2.5 h-2 md:h-2.5 rounded-full transition-all duration-500",
              interactionState !== "Idle" || isEngaged
                ? "bg-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.6)] animate-pulse"
                : "bg-[rgb(var(--foreground-muted))] opacity-60"
            )} />
            <span className="text-[11px] font-bold tracking-[0.3em] md:tracking-[0.4em] uppercase shimmer-text">
              {!isEngaged && interactionState === "Idle" && !isSleeping && "System Dormant"}
              {isSleeping && "System Sleeping (Models Offloaded)"}
              {isEngaged && interactionState === "Idle" && pttStatus === 'IDLE' && !isSleeping && "System Ready"}
              {pttStatus === 'RECORDING' && "Recording..."}
              {pttStatus === 'PROCESSING' && "Processing..."}
              {interactionState === "Thinking" && pttStatus === 'IDLE' && "Thinking..."}
              {interactionState === "AssistantSpeaking" && "Responding..."}
              {interactionState === "Listening" && pttStatus === 'IDLE' && "Awaiting Audio..."}
              {interactionState === "UserSpeaking" && pttStatus === 'IDLE' && "Listening..."}
            </span>
          </div>
        </div>

        {/* Dynamic Orb Area */}
        <div className="flex-1 w-full flex items-center justify-center relative min-h-0">
          <div className="absolute inset-0 bg-gradient-radial from-[rgb(var(--accent))]/5 to-transparent pointer-events-none opacity-60" />
          <div className={cn(
            "w-[85vw] h-[85vw] max-w-[460px] max-h-[460px] md:w-[460px] md:h-[460px] transition-all duration-1000 flex items-center justify-center",
            !isEngaged ? "grayscale-[0.8] opacity-60 blur-[2px]" : "grayscale-0 opacity-100 blur-0",
            isSleeping && "grayscale-[0.9] opacity-30 blur-[4px]"
          )}>
            <ErrorBoundary name="VoxOrb">
              <VoxOrb interactionState={interactionState} isSleeping={isSleeping} isTesting={!!testingClip} />
            </ErrorBoundary>
          </div>
        </div>

        {/* Interaction Zone */}
        <div className="p-6 md:p-12 pt-0 w-full flex flex-col items-center shrink-0">
          <div className="w-full max-w-2xl flex items-center justify-center relative h-32 overflow-visible">
            {/* Flanking Waveform Container */}
            <div className={cn(
              "absolute inset-0 flex items-center justify-center transition-all duration-700 pointer-events-none",
              activeSpeaking && !testingClip ? "opacity-600 scale-100" : "opacity-0 scale-95 blur-md"
            )}>
              <LiveWaveform
                active={activeSpeaking && !testingClip}
                processing={false}
                telemetryRef={telemetryRef}
                height={60}
                className="w-full"
                mode="static"
                barWidth={3}
                barGap={2}
                fadeWidth={120} 
              />
            </div>

            {/* Buttons Container — Opaque Background Mask */}
            <div className="flex items-center gap-6 relative z-20 px-8 py-5 rounded-full bg-[rgb(var(--background))]">
              <div className="relative">
                <button
                  onClick={handleEngage}
                  className={cn(
                    "flex items-center justify-center w-16 h-16 rounded-full transition-all duration-500 border-2",
                    (isEngaged && isThinking || isLaunching) && "engage-btn-loading",
                    isLaunching && "animate-spin",
                    isEngaged
                      ? "bg-[rgb(var(--background))] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.3)] text-[rgb(var(--accent))]"
                      : "bg-[rgb(var(--accent))] border-transparent  text-[rgb(var(--accent-foreground))] hover:scale-105 shadow-[0_0_30px_rgba(var(--accent),0.5)]"
                  )}
                  disabled={isLaunching}
                >
                  {isLaunching ? (
                    <Activity size={22} className="animate-pulse" />
                  ) : (
                    <Activity size={22} className={cn("transition-transform duration-700", isEngaged && "rotate-180")} />
                  )}
                </button>
                <div className="absolute -bottom-7 left-1/2 -translate-x-1/2 w-24 text-center">
                  <span className="text-[11px] font-bold tracking-[0.2em] uppercase opacity-60">
                    {isEngaged ? "Stop" : "Engage"}
                  </span>
                </div>
              </div>

              {/* PTT Mic Button — Only visible when Engaged + PTT mode and not testing */}
              {isEngaged && !testingClip && interactionMode === "PTT" && (
                <div className="relative animate-in fade-in slide-in-from-left-4 duration-500">
                  <button
                    onClick={togglePtt}
                    className={cn(
                      "flex items-center justify-center w-16 h-16 rounded-full transition-all duration-500 border-2",
                      pttStatus === 'RECORDING'
                        ? "bg-[rgb(var(--accent))] border-transparent  scale-110 text-[rgb(var(--accent-foreground))]"
                        : "bg-[rgb(var(--background))] border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] hover:border-[rgb(var(--accent))] "
                    )}
                  >
                    <Mic size={24} className={cn(pttStatus === 'RECORDING' && "animate-pulse")} />
                  </button>
                  <div className="absolute -bottom-7 left-1/2 -translate-x-1/2 w-24 text-center">
                    <span className={cn(
                      "text-[11px] font-black tracking-[0.3em] uppercase transition-colors",
                      pttStatus === 'RECORDING' ? "text-[rgb(var(--accent))]" : "opacity-60"
                    )}>
                      {pttStatus === 'RECORDING' ? "Live" : "MIC"}
                    </span>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* ===== RIGHT SIDEBAR BRIEF (Desktop Only) ===== */}
      <div className="hidden xl:flex flex-col gap-6 py-16 pr-12 w-[420px] shrink-0 z-10">
        <div className="glass-card p-10 min-h-[500px] flex flex-col relative group">
          <div className="absolute top-5 right-5 p-4 opacity-20 group-hover:opacity-40 transition-opacity">
            <Mic size={48} />
          </div>

          <div className="flex items-center gap-3 mb-10 shrink-0">
            <div className="w-1 h-8 bg-[rgb(var(--accent))] rounded-full" />
            <span className="text-[11px] font-bold tracking-[0.3em] text-[rgb(var(--accent))] uppercase">Interaction Stream</span>
          </div>

          <div className="flex-1 flex flex-col gap-8">
            <div className="flex-1 flex flex-col">
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] mb-6 opacity-60">Live Dialogue</h3>
              
              <div 
                ref={scrollRef}
                className="flex-1 flex flex-col gap-6 max-h-[340px] overflow-y-auto custom-scrollbar pr-2"
              >
                {/* User Bubble */}
                <div className={cn(
                  "transition-all duration-500 transform",
                  transcript ? "opacity-100 translate-x-0" : "h-0 opacity-0 -translate-x-4 pointer-events-none overflow-hidden"
                )}>
                  <div className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-2">You</div>
                  <div className="p-4 rounded-2xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.05)]">
                    <p className="text-lg font-medium text-[rgb(var(--foreground))] leading-relaxed">
                      {transcript}
                    </p>
                  </div>
                </div>

                {/* Assistant Bubble */}
                <div className={cn(
                  "transition-all duration-700 delay-200 transform",
                  assistantText ? "opacity-100 translate-y-0" : "h-0 opacity-0 translate-y-4 pointer-events-none overflow-hidden"
                )}>
                  <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2">Vox</div>
                  <div className="p-4 rounded-2xl bg-[rgb(var(--accent))]/[0.03] border border-[rgb(var(--accent))]/10">
                    <p className="text-lg font-medium text-[rgb(var(--accent))] leading-relaxed">
                      {assistantText}
                    </p>
                  </div>
                </div>

                {!transcript && !assistantText && (
                  <div className="flex-1 flex flex-col items-center justify-center text-center px-6 opacity-60">
                    <Activity size={32} className="mb-4 animate-pulse" />
                    <p className="text-sm font-medium italic">
                      {isEngaged 
                        ? (interactionMode === "PTT" ? "Click the Mic button to start recording" : "Listening for your voice...") 
                        : "System dormant. Click Engage to start."}
                    </p>
                  </div>
                )}
              </div>
            </div>

            <div className="pt-8 border-t border-[rgba(var(--border),0.05)] grid grid-cols-2 gap-8 shrink-0">
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-60">Pipeline</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))] uppercase">
                  {isEngaged ? "Active" : "Locked"}
                </div>
              </div>
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-60">Protocol</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))] uppercase">
                  {interactionMode}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* ===== TEST MODE (full width, below sidebar) ===== */}
        <div className="relative w-full">
          <div className={cn(
            "premium-card w-full transition-all duration-300 overflow-hidden",
            isEngaged && !testingClip ? "opacity-60 cursor-not-allowed" : "hover:border-[rgb(var(--accent))]/40"
          )}>
            {/* Header/Button part */}
            <button
              onClick={() => !isEngaged && setTestMode(!testMode)}
              disabled={isEngaged && !testingClip}
              className={cn(
                "w-full p-4 flex items-center justify-between transition-all duration-300",
                isEngaged && !testingClip ? "cursor-not-allowed" : "cursor-pointer"
              )}
            >
              <div className="flex items-center gap-3">
                <FlaskConical size={16} className={cn(
                  "transition-all duration-300",
                  testingClip ? "text-[rgb(var(--accent))] animate-pulse" : testMode ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]"
                )} />
                <div className="text-left">
                  <div className="text-[11px] font-bold tracking-widest uppercase text-[rgb(var(--foreground-muted))]">
                    {testingClip ? "Test In Progress" : "Test Mode"}
                  </div>
                  {testingClip && (
                    <div className="flex items-center gap-2 text-[13px] font-semibold text-[rgb(var(--accent))] mt-0.5 animate-pulse">
                      <div className="w-2.5 h-2.5 rounded-full border-2 border-[rgb(var(--accent))] border-t-transparent animate-spin shrink-0" />
                      <span className="truncate max-w-[200px] block">
                        Running: {testClips.find(c => c.id === testingClip)?.name}
                      </span>
                    </div>
                  )}
                </div>
              </div>
              {!isEngaged && (
                <div className={cn(
                  "w-4 h-4 rounded-full border-2 transition-all duration-500",
                  testMode
                    ? "bg-[rgb(var(--accent))] border-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.4)]"
                    : "border-[rgb(var(--foreground-muted))]/30"
                )} />
              )}
            </button>

            {/* Expandable clips list */}
            {testMode && !isEngaged && (
              <div className="border-t border-[rgba(var(--border),0.05)] p-2 bg-[rgb(var(--foreground))]/[0.01] animate-in slide-in-from-top-2 duration-300">
                <div className="flex flex-col gap-1">
                  {testClips.map((clip) => (
                    <div
                      key={clip.id}
                      className="flex items-center justify-between gap-3 p-3 rounded-xl hover:bg-[rgb(var(--foreground))]/[0.03] transition-all duration-300"
                    >
                      <div className="flex-1 min-w-0">
                        <div className="text-[13px] font-semibold text-[rgb(var(--foreground))] truncate">
                          {clip.name}
                        </div>
                        <div className="text-[11px] text-[rgb(var(--foreground-muted))] truncate opacity-70">
                          {clip.desc} · {clip.duration}
                        </div>
                      </div>

                      <button
                        onClick={() => handleTestClip(clip.id)}
                        className="shrink-0 px-3 py-1.5 rounded-lg text-[11px] font-bold tracking-wider uppercase transition-all duration-300 bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/20"
                      >
                        Test
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
