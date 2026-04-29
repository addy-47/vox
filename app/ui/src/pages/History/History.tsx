import React, { useState } from "react";
import { cn } from "../../shared/lib/utils";
import { Clock, Mic, ChevronRight, Activity, Database, Search } from "lucide-react";

export const History: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'transcripts' | 'conversations'>('transcripts');

  return (
    <div className="flex h-full w-full bg-[rgb(var(--background))] overflow-hidden relative">
      {/* Background Decor - Ambient Glow */}
      <div className="absolute top-[-10%] right-[-10%] w-[50%] h-[50%] bg-[rgb(var(--accent))]/5 blur-[120px] rounded-full pointer-events-none" />
      <div className="absolute bottom-[-10%] left-[-5%] w-[40%] h-[40%] bg-[rgb(var(--accent))]/3 blur-[100px] rounded-full pointer-events-none" />
      
      <div className="flex-1 flex flex-col min-w-0 p-8 md:p-12 lg:p-16 z-10">
        {/* Header Section - Asymmetric Alignment */}
        <header className="mb-16 flex flex-col lg:flex-row lg:items-end justify-between gap-10">
          <div className="space-y-6 max-w-2xl">
            <div className="flex items-center gap-4">
              <div className="w-1.5 h-10 bg-[rgb(var(--accent))] rounded-full shadow-[0_0_15px_rgba(var(--accent),0.5)]" />
              <h1 className="text-4xl font-bold tracking-tighter text-[rgb(var(--foreground))] lg:text-5xl">
                Activity <span className="text-[rgb(var(--accent))] opacity-80 italic font-light">Logs</span>
              </h1>
            </div>
            <p className="text-[rgb(var(--foreground-muted))] text-sm lg:text-base leading-relaxed opacity-70 tracking-wide">
              A distributed chronological index of neural-voice interactions and system-level transcription events captured by the VOX gateway.
            </p>
          </div>

          {/* Controls - Search & Tabs */}
          <div className="flex flex-col sm:flex-row items-center gap-4">
             {/* Search Bar - Premium Input */}
            <div className="relative group w-full sm:w-64">
              <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[rgb(var(--foreground-muted))] group-focus-within:text-[rgb(var(--accent))] transition-colors" />
              <input 
                type="text" 
                placeholder="Search logs..." 
                className="w-full bg-white/[0.03] border border-white/10 rounded-2xl py-3 pl-12 pr-4 text-xs font-medium text-[rgb(var(--foreground))] focus:outline-none focus:border-[rgb(var(--accent))]/50 focus:ring-4 focus:ring-[rgb(var(--accent))]/5 transition-all"
              />
            </div>

            {/* Tab Switcher - Floating Pill Design */}
            <div className="flex p-1.5 rounded-2xl bg-white/[0.03] border border-white/10 backdrop-blur-xl shrink-0">
              {(['transcripts', 'conversations'] as const).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={cn(
                    "px-8 py-2.5 rounded-xl text-[10px] font-bold tracking-[0.25em] uppercase transition-all duration-500",
                    activeTab === tab 
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-[0_8px_20px_rgba(var(--accent),0.2)]" 
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {tab}
                </button>
              ))}
            </div>
          </div>
        </header>

        {/* Dynamic Log Feed */}
        <div className="flex-1 overflow-y-auto pr-6 -mr-6 custom-scrollbar">
          <div className="flex flex-col gap-8 max-w-6xl">
            {activeTab === 'transcripts' ? (
              <div className="grid grid-cols-1 gap-6 pb-12">
                {[1, 2, 3, 4, 5, 6].map((i) => (
                  <div 
                    key={i} 
                    className={cn(
                      "premium-card p-8 flex flex-col lg:flex-row gap-8 group hover:translate-x-2 transition-all duration-500 cursor-pointer overflow-hidden relative",
                      i % 3 === 0 ? "lg:w-[92%]" : "w-full" // Asymmetry
                    )}
                  >
                    {/* Hover Glow */}
                    <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/0 to-[rgb(var(--accent))]/5 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none" />
                    
                    <div className="flex lg:flex-col items-center lg:items-start justify-between lg:justify-center gap-4 lg:w-32 shrink-0 lg:border-r border-[rgba(var(--border))]">
                      <div className="flex items-center gap-2">
                        <Clock size={14} className="text-[rgb(var(--accent))] opacity-50" />
                        <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))]">14:2{i} PM</span>
                      </div>
                      <div className="text-[9px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 px-2 py-0.5 rounded uppercase tracking-tighter">
                        Captured
                      </div>
                    </div>

                    <div className="flex-1 space-y-4">
                      <div className="flex items-center gap-3">
                        <Activity size={12} className="text-[rgb(var(--accent))] animate-pulse" />
                        <span className="text-[10px] font-bold uppercase tracking-[0.3em] text-[rgb(var(--foreground-muted))] opacity-40">System Transcription</span>
                      </div>
                      <p className="text-[rgb(var(--foreground))] text-lg lg:text-xl font-medium leading-relaxed">
                        {i % 2 === 0 
                          ? "Deploy the latest micro-service architecture for the telemetry dashboard and ensure the Orb animation is optimized for 120fps."
                          : "Audit the current neural node synchronization across all edge devices and report any latency spikes exceeding 15ms."}
                      </p>
                    </div>

                    <div className="flex items-center justify-end gap-6 shrink-0 opacity-0 group-hover:opacity-100 transition-all translate-x-4 group-hover:translate-x-0">
                      <div className="flex flex-col items-end">
                        <span className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 opacity-40">Confidence</span>
                        <div className="text-xs font-mono font-bold text-[rgb(var(--accent))]">99.8%</div>
                      </div>
                      <button className="w-10 h-10 rounded-full bg-white/[0.05] flex items-center justify-center text-[rgb(var(--foreground))] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] transition-colors">
                        <ChevronRight size={18} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 pb-12">
                {[1, 2, 3, 4].map((i) => (
                  <div 
                    key={i} 
                    className={cn(
                      "premium-card p-10 flex flex-col gap-8 group hover:scale-[1.01] transition-all duration-500 overflow-hidden relative",
                      i === 1 ? "lg:col-span-8 lg:row-span-2" : "lg:col-span-4" // Masonry style
                    )}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-4">
                        <div className="w-12 h-12 rounded-2xl bg-[rgb(var(--accent))]/10 flex items-center justify-center text-[rgb(var(--accent))] shadow-inner">
                          <Mic size={24} />
                        </div>
                        <div>
                          <h4 className={cn("font-bold text-[rgb(var(--foreground))]", i === 1 ? "text-xl" : "text-sm")}>Neural Session</h4>
                          <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] opacity-50 uppercase tracking-widest">SID: VOX-00{i}</span>
                        </div>
                      </div>
                      <div className="flex items-center gap-2 bg-white/[0.03] px-3 py-1 rounded-full border border-white/5">
                        <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
                        <span className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-tighter">Active</span>
                      </div>
                    </div>

                    <div className="space-y-6 flex-1">
                      <div className="bg-white/[0.02] rounded-2xl p-6 border border-white/10 relative overflow-hidden group-hover:border-[rgb(var(--accent))]/20 transition-colors">
                        <div className="absolute top-0 left-0 w-1 h-full bg-[rgb(var(--accent))] opacity-20" />
                        <p className={cn("text-[rgb(var(--foreground-muted))] leading-relaxed italic", i === 1 ? "text-base" : "text-sm")}>
                          {i === 1 
                            ? "\"Initiating system-wide audit of all neural nodes. Architectural synthesis starting in 3... 2... 1... Latency check completed. All nodes reporting nominal status.\""
                            : "\"Interaction logs show high engagement with the architectural visualization module. Recommendations: Increase orb hollow space.\""}
                        </p>
                      </div>

                      {i === 1 && (
                        <div className="grid grid-cols-3 gap-6 pt-6 border-t border-[rgba(var(--border))]">
                          <div>
                            <div className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 opacity-40">Duration</div>
                            <div className="text-sm font-mono font-bold text-[rgb(var(--accent))]">14m 22s</div>
                          </div>
                          <div>
                            <div className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 opacity-40">Tokens</div>
                            <div className="text-sm font-mono font-bold text-[rgb(var(--accent))]">8.4k</div>
                          </div>
                          <div>
                            <div className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 opacity-40">Entity Match</div>
                            <div className="text-sm font-mono font-bold text-[rgb(var(--accent))]">12</div>
                          </div>
                        </div>
                      )}
                    </div>

                    <button className="flex items-center gap-2 text-[10px] font-bold text-[rgb(var(--accent))] uppercase tracking-[0.2em] group-hover:gap-4 transition-all mt-4 self-start">
                      View Session Details <ChevronRight size={14} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
