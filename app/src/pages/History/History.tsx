import React, { useState } from "react";
import { Filter, Search, Trash2, ChevronRight, Activity, Clock, MessageSquare, ChevronDown } from "lucide-react";
import { cn } from "../../shared/lib/utils";

const sttHistory = [
  { id: 1, text: "The architectural synthesis of neural networks requires high-fidelity data.", time: "12:45 PM", duration: "1.2s", confidence: "98%" },
  { id: 2, text: "Initiate system purge of local biometric signatures.", time: "11:20 AM", duration: "0.8s", confidence: "99%" },
  { id: 3, text: "Increase the hollow space within the central orb visualizer.", time: "10:15 AM", duration: "2.1s", confidence: "94%" },
  { id: 4, text: "Enable distributed neural indexing for gateway 0.1.", time: "09:30 AM", duration: "1.5s", confidence: "97%" },
  { id: 5, text: "Switch vocal profile to ETHER synthesis.", time: "08:12 AM", duration: "0.9s", confidence: "96%" },
];

const conversationHistory = [
  { 
    id: "conv-1", 
    title: "System Architecture Discussion", 
    time: "Apr 29, 02:30 PM", 
    messages: [
      { role: "user", text: "How does the neural engine handle real-time STT?" },
      { role: "vox", text: "The VOX engine utilizes a low-latency transformer model optimized for edge inference." },
      { role: "user", text: "What about the VAD logic?" },
      { role: "vox", text: "VAD is handled via a dedicated passive monitor layer that triggers the neural link." }
    ]
  },
  { 
    id: "conv-2", 
    title: "Biometric Purge Protocol", 
    time: "Apr 28, 11:00 AM", 
    messages: [
      { role: "user", text: "Start the purge protocol." },
      { role: "vox", text: "Biometric data purging initiated. All local signatures cleared." }
    ]
  }
];

export const History: React.FC = () => {
  const [activeTab, setTab] = useState<"stt" | "chat">("stt");
  const [expandedSession, setExpandedSession] = useState<string | null>(null);

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative">
      
      {/* Page header - Identical to Settings */}
      <header className="p-6 md:p-12 border-b border-[rgba(var(--border),0.05)] glass-panel shrink-0">
        <div className="max-w-7xl mx-auto w-full flex flex-col md:flex-row md:items-center justify-between gap-8">
          <div className="space-y-4">
            <div className="flex items-center gap-2 mb-1">
              <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
              <span className="text-[11px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase">Telemetry</span>
            </div>
            <h1 className="text-3xl md:text-4xl font-bold tracking-tight text-[rgb(var(--foreground))]">
              Activity <span className="text-[rgb(var(--foreground-muted))] opacity-40">Logs</span>
            </h1>
          </div>

          <div className="flex flex-wrap items-center gap-4">
            <button 
              onClick={() => setTab("stt")}
              className={cn(
                "px-6 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all",
                activeTab === "stt" 
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg shadow-[rgb(var(--accent))]/20" 
                  : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-white/10 hover:bg-white/10"
              )}
            >
              STT History
            </button>
            <button 
              onClick={() => setTab("chat")}
              className={cn(
                "px-6 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all",
                activeTab === "chat" 
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg shadow-[rgb(var(--accent))]/20" 
                  : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-white/10 hover:bg-white/10"
              )}
            >
              Conversations
            </button>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="flex-1 overflow-y-auto custom-scrollbar p-6 md:p-12 pb-32 md:pb-12">
        <div className="max-w-7xl mx-auto space-y-8">
          {/* Controls */}
          <div className="flex flex-col md:flex-row gap-4 items-center justify-between">
            <div className="relative w-full md:w-96">
              <Search className="absolute left-4 top-1/2 -translate-y-1/2 text-[rgb(var(--foreground-muted))] opacity-40" size={16} />
              <input 
                type="text" 
                placeholder={activeTab === "stt" ? "SEARCH TRANSCIPTS..." : "SEARCH SESSIONS..."} 
                className="w-full bg-white/[0.03] border border-white/10 rounded-xl py-3 pl-12 pr-4 text-[11px] font-bold uppercase tracking-widest focus:outline-none focus:border-[rgb(var(--accent))]/50 transition-all"
              />
            </div>
            <div className="flex items-center gap-3 w-full md:w-auto">
              <button className="flex-1 md:flex-none flex items-center justify-center gap-2 px-6 py-3 rounded-xl bg-white/[0.03] border border-white/10 text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest hover:text-[rgb(var(--foreground))] transition-all">
                <Filter size={14} /> Filter
              </button>
              <button className="flex-1 md:flex-none flex items-center justify-center gap-2 px-6 py-3 rounded-xl bg-red-500/5 border border-red-500/10 text-[11px] font-bold text-red-400 uppercase tracking-widest hover:bg-red-500/10 transition-all">
                <Trash2 size={14} /> Clear
              </button>
            </div>
          </div>

          {/* List Area */}
          <div className="grid gap-4">
            {activeTab === "stt" ? (
              sttHistory.map((item) => (
                <div 
                  key={item.id}
                  className="premium-card p-6 flex flex-col md:flex-row md:items-center justify-between gap-6 group hover:border-[rgb(var(--accent))]/30 transition-all cursor-pointer"
                >
                  <div className="flex gap-6 items-start flex-1">
                    <div className="p-3 rounded-xl bg-white/[0.03] text-[rgb(var(--accent))] shrink-0">
                      <Activity size={18} />
                    </div>
                    <div className="space-y-3">
                      <p className="text-sm md:text-base leading-relaxed text-[rgb(var(--foreground))] font-medium">
                        {item.text}
                      </p>
                      <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
                        <div className="flex items-center gap-2 text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-40">
                          <Clock size={12} /> {item.time}
                        </div>
                        <div className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--accent))]">
                          Duration: {item.duration}
                        </div>
                        <div className="text-[11px] font-bold uppercase tracking-widest text-emerald-400">
                          Confidence: {item.confidence}
                        </div>
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-4 shrink-0 md:border-l border-white/5 md:pl-6">
                    <button className="p-3 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-red-400 transition-colors">
                      <Trash2 size={16} />
                    </button>
                    <button className="flex items-center justify-center w-10 h-10 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] group-hover:bg-[rgb(var(--accent))] group-hover:text-[rgb(var(--accent-foreground))] transition-all">
                      <ChevronRight size={18} />
                    </button>
                  </div>
                </div>
              ))
            ) : (
              conversationHistory.map((session) => (
                <div 
                  key={session.id}
                  className={cn(
                    "premium-card transition-all duration-500 overflow-hidden",
                    expandedSession === session.id ? "ring-1 ring-[rgb(var(--accent))]/30" : "hover:border-[rgb(var(--accent))]/30"
                  )}
                >
                  <div 
                    onClick={() => setExpandedSession(expandedSession === session.id ? null : session.id)}
                    className="p-6 flex items-center justify-between cursor-pointer group"
                  >
                    <div className="flex items-center gap-6">
                      <div className="p-3 rounded-xl bg-white/[0.03] text-[rgb(var(--accent))]">
                        <MessageSquare size={18} />
                      </div>
                      <div>
                        <h3 className="text-sm font-bold text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] transition-colors">{session.title}</h3>
                        <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-40 mt-1">{session.time}</p>
                      </div>
                    </div>
                    <div className={cn(
                      "transition-transform duration-500",
                      expandedSession === session.id ? "rotate-180" : ""
                    )}>
                      <ChevronDown size={20} className="text-[rgb(var(--foreground-muted))]" />
                    </div>
                  </div>
                  
                  {expandedSession === session.id && (
                    <div className="px-6 pb-6 pt-2 space-y-6 animate-in fade-in slide-in-from-top-2 duration-500">
                      <div className="h-px bg-white/5 w-full" />
                      <div className="space-y-4">
                        {session.messages.map((msg, idx) => (
                          <div key={idx} className={cn(
                            "flex flex-col gap-2",
                            msg.role === "user" ? "items-end" : "items-start"
                          )}>
                            <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-30">
                              {msg.role}
                            </span>
                            <div className={cn(
                              "max-w-[80%] p-4 rounded-2xl text-xs leading-relaxed",
                              msg.role === "user" 
                                ? "bg-[rgb(var(--accent))]/10 text-[rgb(var(--foreground))] border border-[rgb(var(--accent))]/20" 
                                : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-white/5"
                            )}>
                              {msg.text}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
