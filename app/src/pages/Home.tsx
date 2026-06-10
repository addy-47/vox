import React, { useState, useEffect, useRef } from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { ErrorBoundary } from "@/shared/components/ErrorBoundary";
import { LiveWaveform } from "@/shared/components/LiveWaveform";
import { PipelineField } from "@/shared/components/PipelineField";
import { Activity, Mic, FlaskConical } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";
type InteractionMode = "PASSIVE" | "PTT";

export const Home: React.FC = () => {
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
      setTranscript("");
      setAssistantText("");
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
    if (isEngaged) return;

    hasActiveTurnStarted.current = false;
    setTestingClip(clipId);
    setIsEngaged(true);
    setTestMode(false);
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
        
        const settings = await invoke<any>("get_settings");
        if (settings?.main_app_mode) {
          setInteractionMode(settings.main_app_mode.toUpperCase() as InteractionMode);
        }

        try {
          const snapshot = await invoke<any>("get_runtime_snapshot");
          if (snapshot) {
            setIsEngaged(snapshot.is_engaged);
            setIsSleeping(snapshot.is_sleeping);
            if (snapshot.cpu_governor && !snapshot.cpu_governor_optimal) {
              setCpuWarning({
                governor: snapshot.cpu_governor,
                advice: "Switch to 'performance' governor",
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
            setAssistantText("");
            setTranscript("");
          }
        }));

        unlisteners.push(await appWindow.listen<boolean>("auto_sleep_state", (event) => {
          setIsSleeping(event.payload);
        }));

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
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Sentient Field Background Energy */}
      <PipelineField state={interactionState} />

      {/* Floating Status & Warning HUD */}
      <div className="absolute top-4 left-6 z-30 pointer-events-none">
        {cpuWarning && (
          <span className="signal-text text-amber-500/80 animate-pulse text-[9px] tracking-wider uppercase">
            ⚠ CPU: {cpuWarning.governor} (NON-OPTIMAL)
          </span>
        )}
      </div>

      <div className="absolute top-4 right-6 z-30 pointer-events-none flex items-center gap-3">
        {testingClip && (
          <span className="signal-text text-amber-500 animate-pulse">TESTING</span>
        )}
        <span className="signal-text">
          {isSleeping ? "SLEEPING" : isEngaged ? interactionState : "DORMANT"}
        </span>
      </div>

      {/* Floating Dialogue Area (Upper Field) */}
      <div className="absolute top-[12%] left-1/2 -translate-x-1/2 w-full max-w-2xl px-6 flex flex-col items-center justify-center text-center z-20 pointer-events-none select-text">
        {transcript && (
          <p className="field-text mb-4 text-[rgb(var(--foreground))]/70 animate-in fade-in duration-500 font-light">
            {transcript}
          </p>
        )}
        {assistantText && (
          <p className="field-text text-[rgb(var(--accent))] animate-in fade-in duration-500 font-medium tracking-wide">
            {assistantText}
          </p>
        )}
        {!transcript && !assistantText && (
          <p className="ambient-label tracking-[0.25em] text-[rgb(var(--foreground-muted))]/40 animate-pulse">
            {isSleeping 
              ? "Models Offloaded" 
              : isEngaged 
                ? (interactionMode === "PTT" ? "Hold mic button to speak" : "Voice signal active") 
                : "Awaiting Engagement"}
          </p>
        )}
      </div>

      {/* Sentient Orb Stage (Lower Center) */}
      <div className="absolute top-[55%] left-1/2 -translate-x-1/2 -translate-y-1/2 z-10 w-[340px] h-[340px] md:w-[380px] md:h-[380px] flex items-center justify-center pointer-events-none">
        {/* Subtle dynamic ring behind orb */}
        <div className={cn(
          "absolute inset-0 rounded-full border border-[rgb(var(--accent))]/10 transition-all duration-1000",
          isEngaged ? "scale-100 opacity-100 animate-field-pulse" : "scale-90 opacity-20"
        )} />
        <div className="relative w-full h-full flex items-center justify-center">
          <ErrorBoundary name="VoxOrb">
            <VoxOrb interactionState={interactionState} isSleeping={isSleeping} isTesting={!!testingClip} />
          </ErrorBoundary>
        </div>
      </div>

      {/* Interaction Controls Area (Floating bottom) */}
      <div className="absolute bottom-[2%] left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-4 w-full max-w-md">
        
        {/* Waveform under orb / above buttons */}
        <div className={cn(
          "w-full h-10 transition-all duration-500 pointer-events-none opacity-0 scale-95",
          activeSpeaking && !testingClip && "opacity-75 scale-100"
        )}>
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

        {/* Buttons layout */}
        <div className="flex items-center gap-4 relative">
          
          {/* Test Mode Glyph Button */}
          {!isEngaged && (
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

              {/* Expandable test clips list */}
              {testMode && (
                <div className="absolute bottom-14 left-1/2 -translate-x-1/2 w-56 p-2 rounded-2xl bg-black/90 border border-[rgba(var(--accent),0.25)] shadow-[0_10px_30px_rgba(0,0,0,0.6)] backdrop-blur-xl animate-in slide-in-from-bottom-2 duration-300 flex flex-col gap-1 z-30">
                  <div className="px-2 py-1 border-b border-[rgba(var(--accent),0.1)] mb-1">
                    <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--accent))] uppercase block">Select Test Input</span>
                  </div>
                  {testClips.map((clip) => (
                    <button
                      key={clip.id}
                      onClick={() => handleTestClip(clip.id)}
                      className="w-full text-left p-2 rounded-xl hover:bg-[rgb(var(--accent))]/10 transition-colors border border-transparent hover:border-[rgb(var(--accent))]/15 flex flex-col"
                    >
                      <span className="text-[12px] font-semibold text-[rgb(var(--foreground))]">{clip.name}</span>
                      <span className="text-[9px] text-[rgb(var(--foreground-muted))] mt-0.5">{clip.desc} · {clip.duration}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Primary Engage Button */}
          <div className="relative">
            <button
              onClick={handleEngage}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/30 bg-black/35 hover:scale-105 active:scale-95 shadow-[0_4px_20px_rgba(0,0,0,0.4)]",
                (isEngaged && isThinking || isLaunching) && "engage-btn-loading border-transparent",
                isLaunching && "animate-spin",
                isEngaged
                  ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.2)] bg-black/45"
                  : "bg-[rgb(var(--accent))]/10 hover:bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]"
              )}
              disabled={isLaunching}
              aria-label={isEngaged ? "Stop Vox" : "Engage Vox"}
            >
              {isLaunching ? (
                <Activity size={20} className="animate-pulse" />
              ) : (
                <Activity size={20} className={cn("transition-transform duration-700", isEngaged && "rotate-180")} />
              )}
            </button>
          </div>

          {/* Cancel Test Button */}
          {testingClip && (
            <button
              onClick={handleCancelTest}
              className="flex items-center justify-center w-11 h-11 rounded-full border border-red-500/30 bg-red-500/10 hover:bg-red-500/20 text-red-400 transition-all duration-300"
              aria-label="Cancel Test"
            >
              <span className="text-[9px] font-mono font-bold tracking-wider uppercase">Stop</span>
            </button>
          )}

          {/* PTT Mic Button */}
          {isEngaged && !testingClip && interactionMode === "PTT" && (
            <button
              onClick={togglePtt}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/30 bg-black/35 hover:scale-105 active:scale-95",
                pttStatus === 'RECORDING'
                  ? "bg-[rgb(var(--accent))] border-transparent text-[rgb(var(--accent-foreground))] shadow-[0_0_20px_rgba(var(--accent),0.4)]"
                  : "text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
              )}
              aria-label="Toggle PTT Microphone"
            >
              <Mic size={22} className={cn(pttStatus === 'RECORDING' && "animate-pulse")} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
