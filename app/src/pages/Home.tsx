import React, { useState, useEffect } from "react";
import { VoxOrb } from "@/shared/components/AdvancedOrb";
import { LiveWaveform } from "@/shared/components/LiveWaveform";
import { Activity, Mic, Shield } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";
type InteractionMode = "PASSIVE" | "PTT";

export const Home: React.FC = () => {
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [interactionMode, setInteractionMode] = useState<InteractionMode>("PASSIVE");
  const [isEngaged, setIsEngaged] = useState(false);
  const [pttStatus, setPttStatus] = useState<'IDLE' | 'RECORDING' | 'PROCESSING'>('IDLE');
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [isSleeping, setIsSleeping] = useState(false);
  const telemetryRef = useTelemetry();

  const isUserSpeaking = interactionState === "UserSpeaking" || pttStatus === 'RECORDING';
  const isThinking = interactionState === "Thinking" || pttStatus === 'PROCESSING';

  // Waveform only reflects user speech activity.
  // Fades out during Thinking/Processing/AssistantSpeaking.
  const activeSpeaking = isUserSpeaking;

  const handleEngage = async () => {
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
          }
        } catch (e) {
          console.warn("[Home] Failed to sync initial engagement state:", e);
        }

        unlisteners.push(await appWindow.listen<InteractionState>("state_changed", (event) => {
          const newState = event.payload;
          setInteractionState(newState);
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

        {/* Status Area */}
        <div className="p-6 md:p-12 pb-0 flex flex-col items-center gap-4 shrink-0">
          <div className="premium-card flex items-center gap-3 px-6 md:px-10 py-2.5 md:py-3">
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
        <div className="flex-1 w-full flex items-center justify-center relative min-h-0 overflow-visible">
          <div className="absolute inset-0 bg-gradient-radial from-[rgb(var(--accent))]/5 to-transparent pointer-events-none opacity-60" />
          <div className="w-full h-full max-h-[60vh] min-h-[300px] flex items-center justify-center">
            <div className={cn(
              "w-full h-full scale-100 transition-all duration-1000 flex items-center justify-center",
              !isEngaged ? "grayscale-[0.8] opacity-60 blur-[2px]" : "grayscale-0 opacity-600 blur-0",
              isSleeping && "grayscale-[0.9] opacity-30 blur-[4px]"
            )}>
              <VoxOrb telemetryRef={telemetryRef} interactionState={interactionState} isSleeping={isSleeping} />
            </div>
          </div>
        </div>

        {/* Interaction Zone */}
        <div className="p-6 md:p-12 pt-0 w-full flex flex-col items-center shrink-0">
          <div className="w-full max-w-2xl flex items-center justify-center relative h-32 overflow-visible">
            {/* Flanking Waveform Container */}
            <div className={cn(
              "absolute inset-0 flex items-center justify-center transition-all duration-700 pointer-events-none",
              activeSpeaking ? "opacity-600 scale-100" : "opacity-0 scale-95 blur-md"
            )}>
              <LiveWaveform
                active={activeSpeaking}
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
                    isEngaged && isThinking && "engage-btn-loading",
                    isEngaged && isThinking
                      ? "bg-[rgb(var(--background))] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.3)] text-[rgb(var(--accent))]"
                      : isEngaged
                      ? "bg-[rgb(var(--background))] border-[rgb(var(--accent))] shadow-[0_0_30px_rgba(var(--accent),0.1)] text-[rgb(var(--accent))]"
                      : "bg-[rgb(var(--accent))] border-transparent  text-[rgb(var(--accent-foreground))] hover:scale-105 shadow-[0_0_30px_rgba(var(--accent),0.5)]"
                  )}
                >
                  <Activity size={22} className={cn("transition-transform duration-700", isEngaged && "rotate-180")} />
                </button>
                <div className="absolute -bottom-7 left-1/2 -translate-x-1/2 w-24 text-center">
                  <span className="text-[11px] font-bold tracking-[0.2em] uppercase opacity-60">
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
        <div className="premium-card p-10 min-h-[500px] flex flex-col relative group">
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
              
              <div className="flex-1 flex flex-col gap-6">
                {/* User Bubble */}
                <div className={cn(
                  "transition-all duration-500 transform",
                  transcript ? "opacity-600 translate-x-0" : "h-0 opacity-0 -translate-x-4 pointer-events-none overflow-hidden"
                )}>
                  <div className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-2">You</div>
                  <div className="p-4 rounded-2xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),[0.05]">
                    <p className="text-lg font-medium text-[rgb(var(--foreground))] leading-relaxed">
                      {transcript}
                    </p>
                  </div>
                </div>

                {/* Assistant Bubble */}
                <div className={cn(
                  "transition-all duration-700 delay-200 transform",
                  assistantText ? "opacity-600 translate-y-0" : "h-0 opacity-0 translate-y-4 pointer-events-none overflow-hidden"
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

            <div className="pt-8 border-t border-[rgba(var(--border),[0.05] grid grid-cols-2 gap-8 shrink-0">
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

        <div className="premium-card p-6 flex items-center justify-between group hover:border-[rgb(var(--accent))]/30 transition-colors">
          <div className="flex items-center gap-4">
            <div className="p-3 rounded-xl bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))] transition-colors">
              <Shield size={18} />
            </div>
            <div>
              <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Security</div>
              <div className="text-sm font-bold text-[rgb(var(--foreground))]">End-to-End Vault</div>
            </div>
          </div>
          <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]" />
        </div>
      </div>
    </div>
  );
};
