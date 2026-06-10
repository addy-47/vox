import React, { useState, useEffect, useCallback } from "react";
import { MessageSquare, Trash2, Check, X, Clock, CalendarDays, Hash, History as HistoryIcon, Ghost } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "@/shared/context/SettingsContext";
import { motion, AnimatePresence } from "framer-motion";
import { ArrowLeft, ChevronRight } from "lucide-react";
import { GlassSkeleton } from "@/shared/components/GlassSkeleton";

interface SessionRow {
  id: number;
  started_at: number;
  ended_at: number | null;
  turn_count: number;
  first_message: string | null;
}

interface TurnRow {
  id: number;
  session_id: number;
  turn_id: number;
  user_text: string;
  assistant_text: string;
  stt_latency_ms: number | null;
  ttft_ms: number | null;
  created_at: number;
}

export const History: React.FC = () => {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [sessionsLoading, setSessionsLoading] = useState(true);
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const { draftSettings, updateDraft } = useSettings();
  const [isMobile, setIsMobile] = useState(typeof window !== 'undefined' ? window.innerWidth < 1024 : false);
  const [showMobileInspector, setShowMobileInspector] = useState(false);

  useEffect(() => {
    const handleResize = () => setIsMobile(window.innerWidth < 1024);
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  const selectSession = (session: SessionRow) => {
    setSelectedSession(session);
    if (isMobile) {
      setShowMobileInspector(true);
    }
  };

  const fetchSessions = useCallback(async () => {
    try {
      const data = await invoke<SessionRow[]>("get_sessions");
      setSessions(data);
      // Removed auto-select so it doesn't force selectedSession on every poll if we implement one
    } catch (e) {
      console.error("Failed to fetch sessions:", e);
    }
  }, []);

  // Initial fetch and auto-select first session
  useEffect(() => {
    const init = async () => {
      setSessionsLoading(true);
      try {
        const data = await invoke<SessionRow[]>("get_sessions");
        setSessions(data);
        if (data.length > 0 && !selectedSession) {
          setSelectedSession(data[0]);
        }
      } catch (e) {
        console.error("Failed to fetch sessions on init:", e);
      } finally {
        setSessionsLoading(false);
      }
    };
    init();
  }, [selectedSession]);

  useEffect(() => {
    if (!selectedSession) {
      setTurns([]);
      return;
    }
    const fetchTurns = async () => {
      setLoading(true);
      try {
        const data = await invoke<TurnRow[]>("get_turns", { sessionId: selectedSession.id });
        setTurns(data);
      } catch (e) {
        console.error("Failed to fetch turns:", e);
      } finally {
        setLoading(false);
      }
    };
    fetchTurns();
  }, [selectedSession]);

  const handleDelete = async (e: React.MouseEvent, id: number) => {
    e.stopPropagation();
    if (confirmDeleteId === id) {
      // Confirm deletion
      try {
        await invoke("delete_session", { id });
        setConfirmDeleteId(null);
        if (selectedSession?.id === id) {
          setSelectedSession(null);
        }
        fetchSessions();
      } catch (e) {
        console.error("Failed to delete session:", e);
      }
    } else {
      setConfirmDeleteId(id);
      // Auto cancel after 3 seconds
      setTimeout(() => {
        setConfirmDeleteId(current => current === id ? null : current);
      }, 3000);
    }
  };

  const handleCancelDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    setConfirmDeleteId(null);
  };
  
  const formatDate = (ms: number) => {
    return new Date(ms).toLocaleString(undefined, {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
    });
  };

  const formatTime = (ms: number) => {
    return new Date(ms).toLocaleTimeString(undefined, {
      hour: '2-digit', minute: '2-digit'
    });
  };

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent">
      
      <header className="px-6 md:px-10 py-6 md:py-10 shrink-0">
        <div className="max-w-[1600px] mx-auto flex flex-col md:flex-row md:items-end justify-between gap-6">
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-xl glass-surface glass-base">
                <HistoryIcon className="text-[rgb(var(--accent))]" size={24} />
              </div>
              <h1 className="text-2xl md:text-3xl font-bold tracking-tight text-[rgb(var(--foreground))]">
                Session <span className="text-[rgb(var(--foreground-muted))] opacity-60 font-medium">Conversations</span>
              </h1>
            </div>
            <p className="text-sm text-[rgb(var(--foreground-muted))] max-w-md">Review and manage past interactions and transcriptions.</p>
          </div>

          <div className="flex items-center gap-2 px-3 py-2 rounded-xl glass-whisper glass-base">
            <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))] opacity-60">
              Privacy
            </span>
            <button
              onClick={() => updateDraft("persistence", "private_mode", !(draftSettings?.persistence.private_mode))}
              className={cn(
                "group relative flex items-center h-8 w-14 px-1 rounded-full transition-all duration-500",
                draftSettings?.persistence.private_mode 
                  ? "bg-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.4)]" 
                  : "bg-[rgb(var(--foreground))]/10 border border-[rgba(var(--border),0.05)]"
              )}
              aria-label={draftSettings?.persistence.private_mode ? "Private Mode Active (No disk writes)" : "Enable Private Mode"}
            >
              <div className={cn(
                "flex items-center justify-center w-6 h-6 rounded-full bg-white shadow-sm transition-all duration-500 transform",
                draftSettings?.persistence.private_mode ? "translate-x-6" : "translate-x-0"
              )}>
                {draftSettings?.persistence.private_mode 
                  ? <Ghost className="text-[rgb(var(--accent))]" size={12} /> 
                  : <Ghost className="text-slate-400" size={12} />
                }
              </div>
            </button>
          </div>
        </div>
      </header>

      {/* Main Content Area: Horizontal Layout with same proportions as Settings */}
      <div className="flex-1 overflow-hidden relative px-6 md:px-10">
        <div className="h-full max-w-[1600px] mx-auto py-6 md:py-8">
          <div className="grid lg:grid-cols-3 gap-8 h-full items-start pb-4">
            
            {/* Left Column: Sessions List (1/3 Width) */}
            <div className="lg:col-span-1 flex flex-col h-full overflow-hidden">
              <div className="flex-1 overflow-y-auto custom-scrollbar p-1 space-y-3">
                {sessionsLoading ? (
                  <div className="space-y-3">
                    {Array.from({ length: 4 }).map((_, i) => (
                      <GlassSkeleton key={i} variant="card" />
                    ))}
                  </div>
                ) : sessions.length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-16 gap-3">
                    <Ghost size={32} className="text-[rgb(var(--foreground-muted))] opacity-30" />
                    <span className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-50">
                      No sessions yet
                    </span>
                  </div>
                ) : (
                sessions.map(session => (
                  <div 
                    key={session.id}
                    onClick={() => selectSession(session)}
                    className={cn(
                      "group relative p-4 rounded-2xl cursor-pointer transition-all duration-300 border glass-base",
                      selectedSession?.id === session.id 
                        ? "glass-card border-[rgb(var(--accent))]/30 shadow-[0_4px_24px_-4px_rgba(var(--accent),0.15)]" 
                        : "glass-whisper border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                    )}
                  >
                    <div className="flex items-start justify-between gap-3 mb-3">
                      <div className="flex items-center gap-2 min-w-0">
                        <MessageSquare size={14} className={cn(
                          "shrink-0",
                          selectedSession?.id === session.id ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] opacity-60"
                        )} />
                        <h3 className={cn(
                          "text-sm font-bold truncate transition-colors",
                          selectedSession?.id === session.id ? "text-[rgb(var(--foreground))]" : "text-[rgb(var(--foreground))] opacity-80"
                        )}>
                          {session.first_message || "New Session"}
                        </h3>
                      </div>
                      
                      <div className="shrink-0 transition-opacity">
                        {confirmDeleteId === session.id ? (
                          <div className="flex items-center gap-2 glass-whisper glass-base p-1 rounded-lg animate-in fade-in zoom-in duration-200">
                            <button 
                              onClick={(e) => handleDelete(e, session.id)}
                              className="p-1.5 rounded-md text-emerald-400 hover:bg-emerald-400/10 transition-colors"
                              aria-label="Confirm Delete"
                            >
                              <Check size={14} strokeWidth={3} />
                            </button>
                            <button 
                              onClick={handleCancelDelete}
                              className="p-1.5 rounded-md text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-400/10 transition-colors"
                              aria-label="Cancel"
                            >
                              <X size={14} strokeWidth={3} />
                            </button>
                          </div>
                        ) : (
                          <button 
                            onClick={(e) => handleDelete(e, session.id)}
                            className={cn(
                              "p-2 rounded-lg transition-all",
                              selectedSession?.id === session.id
                                ? "text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-400/10"
                                : "opacity-0 group-hover:opacity-60 text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-400/10"
                            )}
                          >
                            <Trash2 size={14} />
                          </button>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center justify-between gap-4">
                      <div className="flex items-center gap-3">
                        <div className="inline-flex items-center gap-1.5 px-2 py-1 rounded-lg glass-whisper glass-base text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-70">
                          <CalendarDays size={10} />
                          {formatDate(session.started_at)}
                        </div>
                        <div className="inline-flex items-center gap-1.5 px-2 py-1 rounded-lg glass-whisper glass-base text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-70">
                          <Hash size={10} />
                          {session.turn_count} {session.turn_count === 1 ? 'Turn' : 'Turns'}
                        </div>
                      </div>
                      
                      {isMobile && (
                        <ChevronRight size={14} className="text-[rgb(var(--accent))] opacity-60" />
                      )}
                    </div>
                  </div>
                )))}
              </div>
            </div>
            {/* Right Column: Chat/Turns List (2/3 Width) */}
            <AnimatePresence mode="wait">
              {(!isMobile || showMobileInspector) && (
                <motion.div 
                  key={selectedSession?.id || 'empty'}
                  initial={{ x: '100%' }}
                  animate={{ x: 0 }}
                  exit={{ x: '100%' }}
                  transition={{ type: 'spring', damping: 25, stiffness: 200 }}
                  className={cn(
                    "fixed inset-x-0 top-8 bottom-[64px] z-50 flex flex-col bg-transparent lg:static lg:flex lg:translate-x-0 lg:col-span-2 lg:rounded-3xl lg:glass-surface lg:glass-base"
                  )}
                >
                  {/* Mobile Header */}
                  {isMobile && (
                    <header className="flex items-center justify-between px-6 py-4 border-b border-[rgba(var(--accent),0.06)] glass-surface glass-base z-10">
                      <button 
                        onClick={() => setShowMobileInspector(false)}
                        className="flex items-center gap-2 text-sm font-bold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
                      >
                        <ArrowLeft size={18} />
                        Back
                      </button>
                      <div className="flex flex-col items-end">
                        <span className="text-[10px] font-black uppercase tracking-widest opacity-40">Session ID</span>
                        <span className="text-xs font-mono">#{selectedSession?.id}</span>
                      </div>
                    </header>
                  )}

                  {!selectedSession ? (
                    <div className="flex-1 flex items-center justify-center">
                      <div className="flex flex-col items-center gap-3 opacity-50">
                        <MessageSquare size={32} className="text-[rgb(var(--accent))]" />
                        <span className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
                          Select a session to view
                        </span>
                      </div>
                    </div>
                  ) : loading ? (
                    <div className="flex-1 flex items-center justify-center">
                      <div className="w-6 h-6 border-2 border-[rgb(var(--accent))]/20 border-t-[rgb(var(--accent))] rounded-full animate-spin" />
                    </div>
                  ) : (
                    <div className="flex-1 overflow-y-auto custom-scrollbar p-6 md:p-10">
                      <div className="max-w-5xl mx-auto space-y-8 pb-12">
                        
                        <div className="text-center pb-6 border-b border-[rgba(var(--accent),0.06)] mb-6">
                          <h2 className="text-xl font-bold text-[rgb(var(--foreground))] mb-4">
                             {selectedSession.first_message || "Session Started"}
                          </h2>
                          <div className="inline-flex items-center gap-3 px-4 py-2 rounded-full glass-whisper glass-base text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-80">
                            <div className="flex items-center gap-1.5">
                              <Clock size={10} />
                              {formatDate(selectedSession.started_at)}
                            </div>
                            {selectedSession.ended_at && (
                               <>
                                 <div className="w-1 h-1 rounded-full bg-[rgb(var(--foreground-muted))]/30" />
                                 <div className="flex items-center gap-1.5">
                                   {((selectedSession.ended_at - selectedSession.started_at) / 1000).toFixed(1)}s
                                 </div>
                               </>
                            )}
                          </div>
                        </div>

                        <div className="space-y-6">
                          {turns.map((turn) => (
                            <div key={turn.id} className="space-y-6">
                              {/* User Message — glass-surface with accent */}
                              <div className="flex flex-col items-end gap-2">
                                <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))] opacity-50">
                                  User · {formatTime(turn.created_at)}
                                </span>
                                <div className="max-w-[85%] px-5 py-3.5 rounded-2xl rounded-tr-sm glass-surface glass-base border border-[rgb(var(--accent))]/20 text-sm md:text-base leading-relaxed text-[rgb(var(--foreground))]">
                                  {turn.user_text}
                                </div>
                              </div>

                              {/* Assistant Message — glass-whisper with accent label */}
                              <div className="flex flex-col items-start gap-2">
                                <span className="flex items-center gap-2 text-[10px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))] opacity-80">
                                  <div className="w-1 h-3 bg-[rgb(var(--accent))] rounded-full" />
                                  Vox
                                </span>
                                <div className="max-w-[85%] px-5 py-3.5 rounded-2xl rounded-tl-sm glass-whisper glass-base border border-[rgba(var(--border),0.06)] text-sm md:text-base leading-relaxed whitespace-pre-wrap text-[rgb(var(--foreground))]">
                                  {turn.assistant_text}
                                </div>
                                {/* Turn Metadata */}
                                <div className="flex items-center gap-3 pl-2">
                                  {turn.stt_latency_ms !== null && (
                                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md glass-whisper glass-base text-[9px] font-mono tracking-wider text-[rgb(var(--foreground-muted))] opacity-60">
                                      STT {turn.stt_latency_ms}ms
                                    </span>
                                  )}
                                  {turn.ttft_ms !== null && (
                                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md glass-whisper glass-base text-[9px] font-mono tracking-wider text-[rgb(var(--foreground-muted))] opacity-60">
                                      TTFT {turn.ttft_ms}ms
                                    </span>
                                  )}
                                </div>
                              </div>
                            </div>
                          ))}
                          {turns.length === 0 && (
                            <div className="flex flex-col items-center justify-center text-center p-8 opacity-40 min-h-[200px]">
                              <MessageSquare size={24} className="mb-3 text-[rgb(var(--foreground-muted))]" />
                              <span className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
                                No conversation data
                              </span>
                            </div>
                          )}
                        </div>

                      </div>
                    </div>
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </div>
    </div>
  );
};
