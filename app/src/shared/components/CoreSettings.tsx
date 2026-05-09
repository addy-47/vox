import React from "react";
import { Brain, ChevronDown, Volume2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface CoreSettingsProps {
  selectedModel: string;
  selectedVoice: string;
  setSelectedVoice: (voice: string) => void;
  interactionMode: "PASSIVE" | "PTT";
  handleInteractionModeChange: (mode: "PASSIVE" | "PTT") => void;
  voices: string[];
}

export const CoreSettings: React.FC<CoreSettingsProps> = ({
  selectedModel,
  selectedVoice,
  setSelectedVoice,
  interactionMode,
  handleInteractionModeChange,
  voices,
}) => {
  return (
    <div className="grid lg:grid-cols-3 gap-8">
      {/* Main Settings Column */}
      <div className="lg:col-span-2 space-y-8">
        {/* AI Model Selection */}
        <div className="premium-card p-6 md:p-8">
          <div className="flex items-center justify-between mb-8">
            <div className="space-y-1">
              <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Neural Engine</h2>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Architectural Model Layer</p>
            </div>
            <Brain className="text-[rgb(var(--accent))]" size={20} />
          </div>

          <div className="grid md:grid-cols-2 gap-6">
            <div className="space-y-3">
              <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40">Active Inference</label>
              <div className="flex items-center justify-between px-5 py-4 rounded-xl bg-white/[0.03] border border-[rgba(var(--border),0.05)] cursor-pointer hover:border-[rgb(var(--accent))]/30 transition-colors duration-300 group">
                <span className="text-xs font-mono text-[rgb(var(--foreground))] opacity-80">{selectedModel}</span>
                <ChevronDown size={14} className="text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))]" />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="p-4 md:p-5 rounded-xl bg-white/[0.03] border border-[rgba(var(--border),0.03)]">
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase mb-2 opacity-40">Latency</div>
                <div className="text-sm font-bold text-[rgb(var(--accent))]">24ms</div>
              </div>
              <div className="p-4 md:p-5 rounded-xl bg-white/[0.03] border border-[rgba(var(--border),0.03)]">
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase mb-2 opacity-40">Status</div>
                <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-60">Stable</div>
              </div>
            </div>
          </div>
        </div>

        {/* Voice Profile */}
        <div className="premium-card p-6 md:p-8">
          <div className="flex items-center justify-between mb-8">
            <div className="space-y-1">
              <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Vocal Profile</h2>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Acoustic Output Parameters</p>
            </div>
            <Volume2 className="text-[rgb(var(--accent))]" size={20} />
          </div>

          <div className="flex flex-wrap gap-3 mb-8">
            {voices.map((v) => (
              <button
                key={v}
                onClick={() => setSelectedVoice(v)}
                className={cn(
                  "px-5 py-2 rounded-xl text-[11px] font-bold tracking-[0.15em] uppercase transition-colors duration-300",
                  selectedVoice === v 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm" 
                    : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-white/10"
                )}
              >
                {v}
              </button>
            ))}
          </div>

          <div className="h-20 w-full bg-white/[0.02] border border-[rgba(var(--border),0.03)] rounded-2xl flex items-center justify-center overflow-hidden">
            <div className="flex items-center gap-1.5">
              {[4, 12, 24, 42, 32, 18, 48, 36, 12, 6].map((h, i) => (
                <div key={i} className="w-1 rounded-full bg-[rgb(var(--accent))]/40 animate-pulse" style={{ height: h, animationDelay: `${i * 0.1}s` }} />
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Sidebar Settings Column */}
      <div className="space-y-8">
        {/* Interaction Toggles */}
        <div className="premium-card p-6 md:p-8 space-y-8">
          <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40 mb-2">Interactions</h3>
          
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80">Interaction Mode</div>
              <div className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-50">{interactionMode === "PASSIVE" ? "Passive VAD Monitor" : "Push-To-Talk Manual"}</div>
            </div>
            <div className="flex p-1 bg-white/[0.05] border border-[rgba(var(--border),0.05)] rounded-xl">
              <button 
                onClick={() => handleInteractionModeChange("PASSIVE")}
                className={cn(
                  "px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase transition-all duration-300",
                  interactionMode === "PASSIVE" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Passive
              </button>
              <button 
                onClick={() => handleInteractionModeChange("PTT")}
                className={cn(
                  "px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase transition-all duration-300",
                  interactionMode === "PTT" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                PTT
              </button>
            </div>
          </div>
          <p className="text-[10px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-40 -mt-4">
            {interactionMode === "PASSIVE" 
              ? "Vox listens continuously and responds when it detects speech. Hands-free, always-on."
              : "Click the mic button to start recording, click again to stop and process. Full control."}
          </p>
        </div>
      </div>
    </div>
  );
};
