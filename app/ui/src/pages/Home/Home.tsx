import React, { useState } from "react";
import { VoxOrb } from "../../shared/ui/AdvancedOrb";
import { LiveWaveform } from "../../shared/ui/LiveWaveform";
import { Activity, Mic, Shield } from "lucide-react";
import { cn } from "../../shared/lib/utils";

export const Home: React.FC = () => {
  const [isListening, setIsListening] = useState(false);

  return (
    <div className="relative flex h-full w-full overflow-hidden">
      {/* ===== CENTRAL HUD AREA ===== */}
      <div className="flex-1 flex flex-col items-center justify-center relative p-4 md:p-10">
        {/* Status indicator */}
        <div className="absolute top-10 flex items-center gap-3 px-6 py-2 rounded-full bg-white/[0.03] border border-white/10 backdrop-blur-md">
          <div className={cn(
            "w-2 h-2 rounded-full animate-pulse",
            isListening ? "bg-[#00dbe9] shadow-[0_0_12px_#00dbe9]" : "bg-white/20"
          )} />
          <span className="text-[11px] font-bold tracking-[0.3em] uppercase shimmer-text">
            {isListening ? "Listening" : "System Ready"}
          </span>
        </div>

        {/* Orb Container - No height restriction to prevent cutoff */}
        <div className="relative flex items-center justify-center w-full grow">
          <VoxOrb amplitude={isListening ? 0.25 : 0.05} frequency={isListening ? 1.8 : 0.8} />
        </div>

        {/* Bottom HUD: Waveform with Centered Button Overlay */}
        <div className="relative w-full max-w-4xl h-32 flex items-center justify-center mb-12">
          {/* Waveform Background */}
          <div className={cn(
            "absolute inset-0 flex items-center justify-center transition-opacity duration-700",
            isListening ? "opacity-100" : "opacity-30"
          )}>
            <LiveWaveform 
              active={isListening} 
              processing={!isListening}
              height={100} 
              className="w-full"
            />
          </div>

          {/* Button Overlay */}
          <button
            onClick={() => setIsListening(!isListening)}
            className={cn(
              "group relative z-10 flex items-center justify-center w-16 h-16 rounded-full transition-all duration-500",
              isListening 
                ? "bg-transparent border-2 border-[#00dbe9] shadow-[0_0_30px_rgba(0,219,233,0.4)] scale-90" 
                : "bg-[#00dbe9] shadow-[0_0_40px_rgba(0,219,233,0.2)] hover:scale-110"
            )}
          >
            <Activity 
              size={28} 
              className={cn(
                "transition-all duration-500",
                isListening ? "text-[#00dbe9] rotate-180" : "text-[#050505]"
              )} 
            />
            {!isListening && (
              <span className="absolute -bottom-8 text-[9px] font-bold tracking-[0.4em] text-[#00dbe9] uppercase">
                Start
              </span>
            )}
          </button>
        </div>
      </div>

      {/* ===== RIGHT SIDEBAR: CONTEXTUAL INFO (Hidden on mobile) ===== */}
      <div className="hidden lg:flex flex-col gap-6 py-12 pr-12 w-[400px] shrink-0 z-10 animate-in fade-in slide-in-from-right duration-700">
        {/* Active Summary Card */}
        <div className="premium-card p-8 cyan-glow bg-gradient-to-br from-white/[0.05] to-transparent">
          <div className="flex items-center justify-between mb-8">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-[#00dbe9]/10">
                <Mic size={16} className="text-[#00dbe9]" />
              </div>
              <span className="text-[11px] font-bold tracking-[0.25em] text-[#00dbe9] uppercase">Summary</span>
            </div>
            <div className="flex gap-1">
              {[1, 2, 3].map(i => (
                <div key={i} className="w-1 h-1 rounded-full bg-[#00dbe9]/40 animate-pulse" style={{ animationDelay: `${i * 200}ms` }} />
              ))}
            </div>
          </div>
          
          <div className="space-y-6">
            <div>
              <h3 className="text-[10px] font-bold text-white/30 uppercase tracking-[0.2em] mb-2">Intent Analysis</h3>
              <p className="text-lg font-medium leading-relaxed text-white/90">
                {isListening 
                  ? "Processing real-time neural signatures..." 
                  : "Awaiting architectural directives for system optimization."}
              </p>
            </div>

            <div className="pt-6 border-t border-white/5">
              <h3 className="text-[10px] font-bold text-white/30 uppercase tracking-[0.2em] mb-3">System Context</h3>
              <div className="grid grid-cols-2 gap-4">
                <div className="p-3 rounded-xl bg-white/[0.02] border border-white/5">
                  <div className="text-[9px] text-white/40 uppercase mb-1">Latency</div>
                  <div className="text-sm font-mono text-[#00dbe9]">18ms</div>
                </div>
                <div className="p-3 rounded-xl bg-white/[0.02] border border-white/5">
                  <div className="text-[9px] text-white/40 uppercase mb-1">Tokens</div>
                  <div className="text-sm font-mono text-[#00dbe9]">1.2k/s</div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Technical Telemetry */}
        <div className="premium-card p-6 border-white/5">
          <div className="flex items-center gap-2 mb-6">
            <Shield size={14} className="text-white/40" />
            <span className="text-[10px] font-bold tracking-[0.2em] text-white/30 uppercase">Telemetry</span>
          </div>
          <div className="space-y-4">
            {[
              { label: "Memory", value: "842MB", status: "Optimal" },
              { label: "Stability", value: "99.9%", status: "Nominal" }
            ].map((stat) => (
              <div key={stat.label} className="flex items-center justify-between">
                <span className="text-[11px] text-white/30">{stat.label}</span>
                <span className="text-[11px] font-mono text-white/70">{stat.value}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
