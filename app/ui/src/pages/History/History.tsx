import React, { useState } from "react";
import { cn } from "../../shared/lib/utils";

export const History: React.FC = () => {
  const [activeTab, setActiveTab] = useState<"transcripts" | "conversations">("transcripts");

  return (
    <div className="flex flex-col h-full w-full max-w-6xl mx-auto px-6 py-12 overflow-hidden">
      {/* Header Area */}
      <div className="flex flex-col md:flex-row md:items-end justify-between gap-6 mb-12">
        <div>
          <h1 className="text-4xl font-bold tracking-tight mb-2 shimmer-text">Activity Logs</h1>
          <p className="text-white/30 text-sm tracking-widest uppercase">System Interaction History</p>
        </div>

        {/* Tab Switcher */}
        <div className="flex p-1 bg-white/[0.03] border border-white/10 rounded-xl">
          {(["transcripts", "conversations"] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={cn(
                "px-6 py-2 rounded-lg text-[10px] font-bold uppercase tracking-[0.2em] transition-all",
                activeTab === tab 
                  ? "bg-[#00dbe9] text-[#050505] shadow-[0_0_20px_rgba(0,219,233,0.3)]" 
                  : "text-white/40 hover:text-white/70"
              )}
            >
              {tab}
            </button>
          ))}
        </div>
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-y-auto pr-4 -mr-4 custom-scrollbar">
        <div className="space-y-6">
          {activeTab === "transcripts" ? (
            // Transcripts View
            <div className="space-y-4">
              {[1, 2, 3, 4, 5].map((i) => (
                <div key={i} className="premium-card p-6 border-white/5 hover:border-white/10 transition-colors">
                  <div className="flex items-center justify-between mb-4">
                    <span className="text-[9px] font-mono text-white/20 uppercase">STT_LOG_00{i}</span>
                    <span className="text-[9px] font-mono text-white/20">Today, 12:{30 + i} PM</span>
                  </div>
                  <p className="text-lg text-white/80 leading-relaxed italic">
                    {i % 2 === 0 
                      ? "\"Initialize the primary research pipeline for the upcoming sprint.\""
                      : "\"Analyze the architectural bottlenecks in the real-time audio stream.\""}
                  </p>
                </div>
              ))}
            </div>
          ) : (
            // Conversations View
            <div className="space-y-6">
              {[1, 2].map((i) => (
                <div key={i} className="premium-card p-8 border-white/5 cyan-glow bg-gradient-to-br from-white/[0.02] to-transparent">
                  <div className="flex items-center gap-3 mb-8">
                    <div className="w-10 h-10 rounded-full bg-[#00dbe9]/10 flex items-center justify-center border border-[#00dbe9]/20">
                      <div className="w-2.5 h-2.5 rounded-full bg-[#00dbe9] animate-pulse" />
                    </div>
                    <div>
                      <h3 className="text-xs font-bold text-white/80 uppercase tracking-widest">Neural Session</h3>
                      <p className="text-[10px] text-white/20 uppercase font-mono">UUID: VOX-550e8400-e29b-41d4-a716-44665544000{i}</p>
                    </div>
                  </div>
                  
                  <div className="space-y-6">
                    <div className="flex gap-6">
                      <div className="text-[10px] font-bold text-[#00dbe9] uppercase mt-1 shrink-0 w-12">User</div>
                      <p className="text-white/70 leading-relaxed">
                        {i === 1 
                          ? "What is the current latency across the websocket bridge?" 
                          : "Generate a summary of the last interaction logs."}
                      </p>
                    </div>
                    
                    <div className="flex gap-6 pt-6 border-t border-white/5">
                      <div className="text-[10px] font-bold text-white/20 uppercase mt-1 shrink-0 w-12">Vox</div>
                      <p className="text-white/90 leading-relaxed">
                        {i === 1 
                          ? "Current bridge latency is stabilized at 18ms. Network jitter is within nominal parameters."
                          : "Interaction logs show high engagement with the architectural visualization module."}
                      </p>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
