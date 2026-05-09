import React, { useState, useEffect } from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { LiveWaveform } from "@/shared/components/LiveWaveform";
import { Activity, Mic, Shield } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useTelemetry } from "@/shared/hooks/useTelemetry";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";
type InteractionMode = "Passive" | "PTT";

export const Home: React.FC = () => {
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("Passive");
  const [isEngaged, setIsEngaged] = useState(false);
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [shouldShowWaveform, setShouldShowWaveform] = useState(true);
  const telemetryRef = useTelemetry();

  const isListening = interactionState === "Listening" || interactionState === "UserSpeaking" || pttStatus === 'RECORDING';
  const isThinking = interactionState === "Thinking" || pttStatus === 'PROCESSING';

  const handleEngage = async () => {
    try {
      if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("engage");
        setIsEngaged(!isEngaged);
        if (!isEngaged) {
          setTranscript("");
          setAssistantText("");
          setShouldShowWaveform(true);
        }
        console.log(!isEngaged ? "[Home] Pipeline engaged." : "[Home] Pipeline disengaged.");
      } else {
        setIsEngaged(!isEngaged);
      }
    } catch (err) {
      console.error("[Home] Engagement failed:", err);
    }
  };

  const togglePtt = async () => {
    if (!isEngaged) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      if (pttStatus === 'IDLE') {
        await invoke("ptt_start");
      } else {
        await invoke("ptt_stop");
      }
    } catch (err) {
      console.error("[Home] PTT toggle failed:", err);
    }
  };

  useEffect(() => {
    let unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          const { invoke } = await import("@tauri-apps/api/core");
          const appWindow = getCurrentWindow();
          
          // Initial Settings
          const settings = await invoke<any>("get_settings");
          if (settings?.interaction_mode) {
            setInteractionMode(settings.interaction_mode);
          }

          unlisteners.push(await appWindow.listen<InteractionState>("state_changed", (event) => {
            const newState = event.payload;
            setInteractionState(newState);
            
            if (newState === "AssistantSpeaking") {
              // Stay active for orb but maybe fade waveform if we want
              // setShouldShowWaveform(false); 
            } else if (newState === "Listening" || newState === "UserSpeaking") {
              setShouldShowWaveform(true);
            }
          }));

          unlisteners.push(await appWindow.listen<{text: string}>("transcript_partial", (event) => {
            setTranscript(event.payload.text);
          }));

          unlisteners.push(await appWindow.listen<{text: string}>("transcript_final", (event) => {
            setTranscript(event.payload.text);
          }));

          unlisteners.push(await appWindow.listen<{text: string}>("llm_chunk", (event) => {
            setAssistantText(prev => prev + event.payload.text);
          }));

          unlisteners.push(await appWindow.listen<string>("mode_changed", (event) => {
            setInteractionMode(event.payload as InteractionMode);
          }));

          unlisteners.push(await appWindow.listen<{ state: string }>("ptt_status", (event) => {
            setPttStatus(event.payload.state as any);
            if (event.payload.state === 'RECORDING') {
              setAssistantText(""); // Clear previous on new turn
              setTranscript("");
            }
          }));

          // Phase 5: Show window only after listeners are ready
          setTimeout(async () => {
            await invoke("show_main_window");
          }, 300);
        }
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
    <div className="flex-1 flex h-full w-full overflow-hidden bg-[rgb(var(--background))] transition-colors duration-300">
      {/* ===== CENTRAL HUD AREA ===== */}
      <div className="flex-1 flex flex-col relative overflow-visible">

        {/* Status Area */}
        <div className="p-6 md:p-12 pb-0 flex flex-col items-center gap-4 shrink-0">
          <div className="premium-card flex items-center gap-3 px-6 md:px-10 py-2.5 md:py-3">
            <div className={cn(
              "w-2 md:w-2.5 h-2 md:h-2.5 rounded-full transition-all duration-500",
              interactionState !== "Idle" || isEngaged
                ? "bg-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.6)] animate-pulse"
                : "bg-[rgb(var(--foreground-muted))] opacity-20"
            )} />
            <span className="text-[11px] font-bold tracking-[0.3em] md:tracking-[0.4em] uppercase shimmer-text">
              {!isEngaged && interactionState === "Idle" && "System Dormant"}
              {isEngaged && interactionState === "Idle" && pttStatus === 'IDLE' && "System Ready"}
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
        <div className="flex-1 w-full flex items-center justify-center relative min-h-0 overflow-visible">
          <div className="absolute inset-0 bg-gradient-radial from-[rgb(var(--accent))]/5 to-transparent pointer-events-none opacity-40" />
          <div className="w-full h-full max-h-[60vh] min-h-[300px] flex items-center justify-center">
            <div className={cn(
              "w-full h-full scale-100 transition-all duration-1000 flex items-center justify-center",
              !isEngaged ? "grayscale-[0.8] opacity-50 blur-[2px]" : "grayscale-0 opacity-100 blur-0"
            )}>
              <VoxOrb telemetryRef={telemetryRef} interactionState={interactionState} />
            </div>
          </div>
        </div>

        {/* Interaction Zone */}
        <div className="p-6 md:p-12 pt-0 w-full flex flex-col items-center shrink-0">
          <div className="w-full max-w-2xl flex items-center justify-center relative h-32 overflow-visible">
            {/* Flanking Waveform Container */}
            <div className={cn(
              "absolute inset-0 flex items-center justify-center transition-all duration-700 pointer-events-none",
              isEngaged && shouldShowWaveform ? "opacity-100 scale-100" : "opacity-0 scale-95 blur-md"
            )}>
              <LiveWaveform
                active={isEngaged && (isListening || interactionState === "AssistantSpeaking")}
                processing={isThinking}
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
            <div className="flex items-center gap-6 relative z-20 px-8 py-5 rounded-full bg-[rgb(var(--background))] border border-white/[0.03]">
              {/* Engage / Stop Button */}
              <div className="relative">
                <button
                  onClick={handleEngage}
                  className={cn(
                    "flex items-center justify-center w-16 h-16 rounded-full transition-all duration-500 border-2",
                    isEngaged
                      ? "bg-[rgb(var(--background))] border-[rgb(var(--accent))] shadow-[0_0_30px_rgba(var(--accent),0.1)] text-[rgb(var(--accent))]"
                      : "bg-[rgb(var(--accent))] border-transparent  text-[rgb(var(--accent-foreground))] hover:scale-105"
                  )}
                >
                  <Activity size={22} className={cn("transition-transform duration-700", isEngaged && "rotate-180")} />
                </button>
                <div className="absolute -bottom-7 left-1/2 -translate-x-1/2 w-24 text-center">
                  <span className="text-[9px] font-bold tracking-[0.2em] uppercase opacity-40">
                    {isEngaged ? "Stop" : "Engage"}
                  </span>
                </div>
              </div>

              {/* PTT Mic Button — Only visible when Engaged + PTT mode */}
              {isEngaged && interactionMode === "PTT" && (
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
                      "text-[9px] font-black tracking-[0.3em] uppercase transition-colors",
                      pttStatus === 'RECORDING' ? "text-[rgb(var(--accent))]" : "opacity-40"
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
        <div className="premium-card p-10 min-h-[500px] flex flex-col relative group">
          <div className="absolute top-0 right-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity">
            <Mic size={48} />
          </div>

          <div className="flex items-center gap-3 mb-10 shrink-0">
            <div className="w-1 h-8 bg-[rgb(var(--accent))] rounded-full" />
            <span className="text-[11px] font-bold tracking-[0.3em] text-[rgb(var(--accent))] uppercase">Interaction Stream</span>
          </div>

          <div className="flex-1 flex flex-col gap-8">
            <div className="flex-1 flex flex-col">
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] mb-6 opacity-50">Live Dialogue</h3>
              
              <div className="space-y-8 flex-1">
                {/* User Bubble */}
                <div className={cn(
                  "transition-all duration-500 transform",
                  transcript ? "opacity-100 translate-x-0" : "opacity-0 -translate-x-4 pointer-events-none"
                )}>
                  <div className="text-[10px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-2">You</div>
                  <div className="p-4 rounded-2xl bg-white/[0.02] border border-white/[0.05]">
                    <p className="text-lg font-medium text-[rgb(var(--foreground))] leading-relaxed">
                      {transcript}
                    </p>
                  </div>
                </div>

                {/* Assistant Bubble */}
                <div className={cn(
                  "transition-all duration-700 delay-200 transform",
                  assistantText ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4 pointer-events-none"
                )}>
                  <div className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2">Vox</div>
                  <div className="p-4 rounded-2xl bg-[rgb(var(--accent))]/[0.03] border border-[rgb(var(--accent))]/10">
                    <p className="text-lg font-medium text-[rgb(var(--accent))] leading-relaxed">
                      {assistantText}
                    </p>
                  </div>
                </div>

                {!transcript && !assistantText && (
                  <div className="h-full flex flex-col items-center justify-center text-center px-6 opacity-30 mt-20">
                    <Activity size={32} className="mb-4 animate-pulse" />
                    <p className="text-sm font-medium italic">
                      {isEngaged 
                        ? (interactionMode === "PTT" ? "Hold the Mic button to speak" : "Listening for your voice...") 
                        : "System dormant. Click Engage to start."}
                    </p>
                  </div>
                )}
              </div>
            </div>

            <div className="pt-8 border-t border-white/[0.05] grid grid-cols-2 gap-8 shrink-0">
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-40">Pipeline</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))] uppercase">
                  {isEngaged ? "Active" : "Locked"}
                </div>
              </div>
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-40">Protocol</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))] uppercase">
                  {interactionMode}
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="premium-card p-6 flex items-center justify-between group hover:border-[rgb(var(--accent))]/30 transition-colors">
          <div className="flex items-center gap-4">
            <div className="p-3 rounded-xl bg-white/[0.03] text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))] transition-colors">
              <Shield size={18} />
            </div>
            <div>
              <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-40">Security</div>
              <div className="text-sm font-bold text-[rgb(var(--foreground))]">End-to-End Vault</div>
            </div>
          </div>
          <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]" />
        </div>
      </div>
    </div>
  );
};
