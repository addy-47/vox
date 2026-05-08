import React, { useState, useEffect } from "react";
import { VoxOrb } from "../../shared/ui/AdvancedOrb";
import { LiveWaveform } from "../../shared/ui/LiveWaveform";
import { Activity, Mic, Shield } from "lucide-react";
import { cn } from "../../shared/lib/utils";
import { useTelemetry } from "../../shared/hooks/useTelemetry";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";

export const Home: React.FC = () => {
  const [interactionState, setInteractionState] = useState<InteractionState>("Idle");
  const [isEngaged, setIsEngaged] = useState(false);
  const [transcript, setTranscript] = useState("");
  const telemetryRef = useTelemetry();
  
  const isListening = interactionState === "Listening" || interactionState === "UserSpeaking";
  const isThinking = interactionState === "Thinking";

  const handleEngage = async () => {
    try {
      if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("engage");
        setIsEngaged(!isEngaged);
        console.log(isEngaged ? "[Home] Pipeline disengaged." : "[Home] Pipeline engaged.");
      } else {
        // Web fallback
        setIsEngaged(!isEngaged);
      }
    } catch (err) {
      console.error("[Home] Engagement failed:", err);
    }
  };

  useEffect(() => {
    let unlistenState: (() => void) | null = null;
    let unlistenPartial: (() => void) | null = null;
    let unlistenFinal: (() => void) | null = null;

    const setup = async () => {
      try {
        if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          const appWindow = getCurrentWindow();
          
          unlistenState = await appWindow.listen<InteractionState>("state_changed", (event) => {
            setInteractionState(event.payload);
          });

          unlistenPartial = await appWindow.listen<{text: string}>("transcript_partial", (event) => {
            setTranscript(event.payload.text);
          });

          unlistenFinal = await appWindow.listen<{text: string}>("transcript_final", (event) => {
            setTranscript(event.payload.text);
          });
        }
      } catch (err) {
        console.error("[Home] Failed to setup Tauri listeners:", err);
      }
    };

    setup();
    return () => {
      if (unlistenState) unlistenState();
      if (unlistenPartial) unlistenPartial();
      if (unlistenFinal) unlistenFinal();
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
              {isEngaged && interactionState === "Idle" && "System Ready"}
              {interactionState === "Listening" && "Awaiting Audio..."}
              {interactionState === "UserSpeaking" && "Listening..."}
              {interactionState === "Thinking" && "Thinking..."}
              {interactionState === "AssistantSpeaking" && "Responding..."}
              {interactionState === "Interrupted" && "Interrupted"}
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
          <div className="w-full max-w-4xl flex flex-col items-center justify-center relative h-40 md:h-48 mb-8 md:mb-0">
            {/* Waveform Layer */}
            <div className={cn(
              "absolute inset-0 flex items-center justify-center transition-all duration-1000",
              isListening ? "opacity-100 scale-100" : "opacity-20 scale-95 blur-sm"
            )}>
              <LiveWaveform
                active={isListening}
                processing={isThinking}
                telemetryRef={telemetryRef}
                height={120}
                className="w-full"
              />
            </div>

            {/* Visual Indicator Layer (Engage Button) */}
            <button
              onClick={handleEngage}
              className={cn(
                "group relative z-20 flex items-center justify-center w-24 h-24 rounded-full transition-all duration-700",
                isEngaged
                  ? "bg-[rgb(var(--background))] border-2 border-[rgb(var(--accent))] shadow-[0_0_50px_rgba(var(--accent),0.2)]"
                  : "bg-[rgb(var(--accent))] shadow-[0_0_60px_rgba(var(--accent),0.4)] hover:scale-110 active:scale-90"
              )}
            >
              <Activity
                size={36}
                className={cn(
                  "transition-all duration-700",
                  isEngaged ? "text-[rgb(var(--accent))] rotate-180" : "text-[rgb(var(--accent-foreground))]"
                )}
              />
              {isEngaged && (
                <div className="absolute -bottom-16 flex flex-col items-center gap-1 animate-pulse text-center">
                  <span className="text-[11px] font-extrabold tracking-[0.5em] text-[rgb(var(--foreground-muted))] uppercase">
                    Stop
                  </span>
                </div>
              )}
              {!isEngaged && (
                <div className="absolute -bottom-16 flex flex-col items-center gap-1 animate-bounce text-center">
                  <span className="text-[11px] font-extrabold tracking-[0.5em] text-[rgb(var(--accent))] uppercase">
                    Engage
                  </span>
                </div>
              )}
            </button>
          </div>
        </div>
      </div>

      {/* ===== RIGHT SIDEBAR BRIEF (Desktop Only) ===== */}
      <div className="hidden xl:flex flex-col gap-6 py-16 pr-12 w-[420px] shrink-0 z-10">
        <div className="premium-card p-10 overflow-hidden relative group">
          <div className="absolute top-0 right-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity">
            <Mic size={48} />
          </div>

          <div className="flex items-center gap-3 mb-10">
            <div className="w-1 h-8 bg-[rgb(var(--accent))] rounded-full" />
            <span className="text-[11px] font-bold tracking-[0.3em] text-[rgb(var(--accent))] uppercase">Live Transcription</span>
          </div>

          <div className="space-y-8 min-h-[200px]">
            <div>
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] mb-4 opacity-50">Current Stream</h3>
              <p className={cn(
                "text-xl font-medium leading-relaxed transition-all duration-500",
                transcript ? "text-[rgb(var(--foreground))]" : "text-[rgb(var(--foreground-muted))] italic"
              )}>
                {transcript || (isEngaged ? "Listening for speech..." : "System dormant. Engage to start transcription.")}
              </p>
            </div>

            <div className="pt-8 border-t border-[rgba(var(--border),0.1)] grid grid-cols-2 gap-8">
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-40">Status</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))] uppercase">
                  {isEngaged ? "Active" : "Passive"}
                </div>
              </div>
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-40">Mode</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))] uppercase">
                  {interactionState}
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
              <div className="text-sm font-bold text-[rgb(var(--foreground))]">Vault Enabled</div>
            </div>
          </div>
          <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
        </div>
      </div>
    </div>
  );
};
