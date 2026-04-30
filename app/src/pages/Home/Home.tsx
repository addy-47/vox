import React, { useState } from "react";
import { VoxOrb } from "../../shared/ui/AdvancedOrb";
import { LiveWaveform } from "../../shared/ui/LiveWaveform";
import { Activity, Mic, Shield } from "lucide-react";
import { cn } from "../../shared/lib/utils";

export const Home: React.FC = () => {
  const [isListening, setIsListening] = useState(false);

  return (
    <div className="flex-1 flex h-full w-full overflow-hidden bg-[rgb(var(--background))] transition-colors duration-300">
      {/* ===== CENTRAL HUD AREA ===== */}
      <div className="flex-1 flex flex-col relative overflow-visible">

        {/* Status Area - Top Aligned matching Header Spacing */}
        <div className="p-6 md:p-12 pb-0 flex flex-col items-center gap-4 shrink-0">
          <div className="premium-card flex items-center gap-3 px-6 md:px-10 py-2.5 md:py-3">
            <div className={cn(
              "w-2 md:w-2.5 h-2 md:h-2.5 rounded-full transition-all duration-500",
              isListening
                ? "bg-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.6)] animate-pulse"
                : "bg-[rgb(var(--foreground-muted))] opacity-20"
            )} />
            <span className="text-[11px] font-bold tracking-[0.3em] md:tracking-[0.4em] uppercase shimmer-text">
              {isListening ? "Streaming Active" : "System Standby"}
            </span>
          </div>
        </div>

        {/* Dynamic Orb Area - Fully Flexible */}
        <div className="flex-1 w-full flex items-center justify-center relative min-h-0 overflow-visible">
          <div className="absolute inset-0 bg-gradient-radial from-[rgb(var(--accent))]/5 to-transparent pointer-events-none opacity-40" />
          <div className="w-full h-full max-h-[60vh] min-h-[300px] flex items-center justify-center">
            <div className="w-full h-full scale-100 transition-transform flex items-center justify-center">
              <VoxOrb amplitude={isListening ? 0.28 : 0.04} frequency={isListening ? 1.6 : 0.6} />
            </div>
          </div>
        </div>

        {/* Interaction Zone - Safe Bottom Aligned */}
        <div className="p-6 md:p-12 pt-0 w-full flex flex-col items-center shrink-0">
          <div className="w-full max-w-4xl flex flex-col items-center justify-center relative h-40 md:h-48 mb-8 md:mb-0">
            {/* Waveform Layer */}
            <div className={cn(
              "absolute inset-0 flex items-center justify-center transition-all duration-1000",
              isListening ? "opacity-100 scale-100" : "opacity-20 scale-95 blur-sm"
            )}>
              <LiveWaveform
                active={isListening}
                processing={!isListening}
                height={120}
                className="w-full"
              />
            </div>

            {/* Button Layer */}
            <button
              onClick={() => setIsListening(!isListening)}
              className={cn(
                "group relative z-20 flex items-center justify-center w-20 h-20 rounded-full transition-all duration-700",
                isListening
                  ? "bg-[rgb(var(--background))] border-2 border-[rgb(var(--accent))] shadow-[0_0_50px_rgba(var(--accent),0.2)] scale-90"
                  : "bg-[rgb(var(--accent))] shadow-[0_0_40px_rgba(var(--accent),0.3)] hover:scale-110 active:scale-95"
              )}
            >
              <Activity
                size={32}
                className={cn(
                  "transition-all duration-700",
                  isListening ? "text-[rgb(var(--accent))] rotate-180" : "text-[rgb(var(--accent-foreground))]"
                )}
              />
              {!isListening && (
                <div className="absolute -bottom-12 flex flex-col items-center gap-1 animate-bounce">
                  <span className="text-[11px] font-bold tracking-[0.5em] text-[rgb(var(--accent))] uppercase">
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
            <span className="text-[11px] font-bold tracking-[0.3em] text-[rgb(var(--accent))] uppercase">Session Brief</span>
          </div>

          <div className="space-y-8">
            <div>
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] mb-4 opacity-50">Active Context</h3>
              <p className="text-xl font-medium leading-relaxed text-[rgb(var(--foreground))]">
                {isListening
                  ? "Neural patterns being indexed for real-time architectural synthesis."
                  : "System idling. Awaiting voice trigger for contextual synchronization."}
              </p>
            </div>

            <div className="pt-8 border-t border-[rgba(var(--border),0.05)] grid grid-cols-2 gap-8">
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-40">Latency</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))]">12ms</div>
              </div>
              <div>
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-2 opacity-40">Throughput</div>
                <div className="text-lg font-mono text-[rgb(var(--accent))]">4.2k</div>
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
