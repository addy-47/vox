import React, { useState } from "react";
import { cn } from "../../shared/lib/utils";
import { Clock, Mic, ChevronRight, Activity, Search, LayoutGrid, List } from "lucide-react";

export const History: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'transcripts' | 'conversations'>('transcripts');

  return (
    <div className="flex h-full w-full bg-[rgb(var(--background))] overflow-hidden relative">
      <div className="flex-1 flex flex-col min-w-0 z-10">
        
        {/* Integrated Header & Tabs */}
        <header className="p-6 md:p-10 lg:p-12 border-b border-[rgba(var(--border))] glass-panel">
          <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-8 max-w-7xl mx-auto w-full">
            <div className="space-y-2">
              <div className="flex items-center gap-3">
                <div className="w-1 h-6 bg-[rgb(var(--accent))] rounded-full" />
                <h1 className="text-2xl md:text-3xl font-bold tracking-tight text-[rgb(var(--foreground))]">
                  Activity <span className="text-[rgb(var(--accent))] opacity-80">Logs</span>
                </h1>
              </div>
              <p className="text-[rgb(var(--foreground-muted))] text-[10px] md:text-xs uppercase tracking-[0.2em] opacity-60">
                Distributed Neural Index • Gateway 0.1
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-4">
              {/* Search - Responsive Width */}
              <div className="relative group w-full md:w-64">
                <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[rgb(var(--foreground-muted))] opacity-50" />
                <input 
                  type="text" 
                  placeholder="Filter logs..." 
                  className="w-full bg-white/[0.03] border border-white/10 rounded-xl py-2.5 pl-10 pr-4 text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground))] focus:outline-none focus:border-[rgb(var(--accent))]/40 transition-all"
                />
              </div>

              {/* Tab Switcher - Integrated in Header Line */}
              <div className="flex p-1 rounded-xl bg-white/[0.03] border border-white/10 backdrop-blur-xl shrink-0">
                {(['transcripts', 'conversations'] as const).map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={cn(
                      "px-6 py-2 rounded-lg text-[9px] font-bold tracking-[0.2em] uppercase transition-all duration-300",
                      activeTab === tab 
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm" 
                        : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    {tab}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </header>

        {/* Dynamic Log Feed - Fixed Scrolling */}
        <div className="flex-1 overflow-y-auto custom-scrollbar bg-gradient-to-b from-[rgb(var(--background))] to-transparent">
          <div className="p-6 md:p-10 lg:p-12 max-w-7xl mx-auto w-full">
            {activeTab === 'transcripts' ? (
              <div className="grid grid-cols-1 gap-4">
                {[1, 2, 3, 4, 5, 6, 7, 8].map((i) => (
                  <div 
                    key={i} 
                    className="premium-card p-6 flex flex-col md:flex-row gap-6 group hover:border-[rgb(var(--accent))]/30 transition-all cursor-pointer"
                  >
                    <div className="flex md:flex-col items-center md:items-start justify-between md:justify-center gap-3 md:w-28 shrink-0 md:border-r border-white/5 pr-6">
                      <div className="flex items-center gap-2">
                        <Clock size={12} className="text-[rgb(var(--accent))]" />
                        <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))]">14:2{i}</span>
                      </div>
                      <span className="text-[8px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded uppercase tracking-tighter">
                        Log_{i}
                      </span>
                    </div>

                    <div className="flex-1">
                      <p className="text-[rgb(var(--foreground))] text-sm md:text-base font-medium leading-relaxed">
                        {i % 2 === 0 
                          ? "Deploy the latest micro-service architecture for the telemetry dashboard and ensure the Orb animation is optimized for 120fps."
                          : "Audit the current neural node synchronization across all edge devices and report any latency spikes exceeding 15ms."}
                      </p>
                    </div>

                    <div className="flex items-center justify-end gap-4 shrink-0 opacity-40 group-hover:opacity-100 transition-opacity">
                      <ChevronRight size={16} className="text-[rgb(var(--foreground-muted))]" />
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {[1, 2, 3, 4, 5].map((i) => (
                  <div 
                    key={i} 
                    className="premium-card p-8 flex flex-col gap-6 group hover:translate-y-[-4px] transition-all"
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-4">
                        <div className="w-10 h-10 rounded-xl bg-[rgb(var(--accent))]/5 flex items-center justify-center text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/10">
                          <Mic size={18} />
                        </div>
                        <div>
                          <h4 className="text-xs font-bold text-[rgb(var(--foreground))] uppercase tracking-widest">Session {i}</h4>
                          <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))] opacity-50 uppercase">UUID_{i}84</span>
                        </div>
                      </div>
                      <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)]" />
                    </div>

                    <div className="flex-1 bg-black/20 dark:bg-white/[0.01] rounded-xl p-4 border border-white/5">
                      <p className="text-[rgb(var(--foreground-muted))] text-xs leading-relaxed italic line-clamp-4 group-hover:line-clamp-none transition-all duration-500">
                        "Initiating system-wide audit of all neural nodes. Architectural synthesis starting in 3... 2... 1... All parameters nominal. Redirecting throughput to primary visualization module."
                      </p>
                    </div>

                    <div className="flex items-center justify-between pt-4 border-t border-white/5">
                      <div className="flex flex-col">
                        <span className="text-[8px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-40">Latency</span>
                        <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]">12ms</span>
                      </div>
                      <button className="text-[9px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest hover:underline">
                        Details
                      </button>
                    </div>
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
