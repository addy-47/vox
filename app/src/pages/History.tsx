import React, { useState, useEffect, useCallback } from "react";
import { MessageSquare, Trash2, Check, X, Clock, CalendarDays, Hash, History as HistoryIcon, Ghost } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "@/shared/context/SettingsContext";

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
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const { draftSettings, updateDraft } = useSettings();

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
      try {
        const data = await invoke<SessionRow[]>("get_sessions");
        setSessions(data);
        if (data.length > 0 && !selectedSession) {
          setSelectedSession(data[0]);
        }
      } catch (e) {
        console.error("Failed to fetch sessions on init:", e);
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
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-[rgb(var(--background))]">
      
      <header className="px-6 md:px-10 py-6 md:py-10 shrink-0">
        <div className="max-w-[1600px] mx-auto flex flex-col md:flex-row md:items-end justify-between gap-6">
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-xl bg-[rgb(var(--accent))]/10">
                <HistoryIcon className="text-[rgb(var(--accent))]" size={24} />
              </div>
              <h1 className="text-2xl md:text-3xl font-bold tracking-tight text-[rgb(var(--foreground))]">
                Session <span className="text-[rgb(var(--foreground-muted))] opacity-60 font-medium">Conversations</span>
              </h1>
            </div>
            <p className="text-sm text-[rgb(var(--foreground-muted))] max-w-md">Review and manage past interactions and transcriptions.</p>
          </div>

          <div className="flex items-center gap-3">
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
              title={draftSettings?.persistence.private_mode ? "Private Mode Active (No disk writes)" : "Enable Private Mode"}
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
                {sessions.map(session => (
                  <div 
                    key={session.id}
                    onClick={() => setSelectedSession(session)}
                    className={cn(
                      "group relative p-4 rounded-2xl cursor-pointer transition-all duration-300 border",
                      selectedSession?.id === session.id 
                        ? "bg-[rgb(var(--foreground))]/[0.04] border-[rgb(var(--accent))]/30 shadow-[0_4px_24px_-4px_rgba(var(--accent),0.1)]" 
                        : "bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/[0.03] hover:border-[rgb(var(--accent))]/20"
                    )}
                  >
                    <div className="flex items-start justify-between gap-3 mb-2">
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
                          <div className="flex items-center gap-2 bg-[rgb(var(--foreground))]/[0.05] p-1 rounded-lg animate-in fade-in zoom-in duration-200">
                            <button 
                              onClick={(e) => handleDelete(e, session.id)}
                              className="p-1.5 rounded-md text-emerald-400 hover:bg-emerald-400/10 transition-colors"
                              title="Confirm Delete"
                            >
                              <Check size={14} strokeWidth={3} />
                            </button>
                            <button 
                              onClick={handleCancelDelete}
                              className="p-1.5 rounded-md text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-400/10 transition-colors"
                              title="Cancel"
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
                                : "opacity-0 group-hover:opacity-600 text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-400/10"
                            )}
                          >
                            <Trash2 size={14} />
                          </button>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center gap-4 text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-60">
                      <div className="flex items-center gap-1.5">
                        <CalendarDays size={12} />
                        {formatDate(session.started_at)}
                      </div>
                      <div className="flex items-center gap-1.5">
                        <Hash size={12} />
                        {session.turn_count} {session.turn_count === 1 ? 'Turn' : 'Turns'}
                      </div>
                    </div>
                  </div>
                ))}
                {sessions.length === 0 && (
                  <div className="text-center p-8 text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-60">
                    No sessions found
                  </div>
                )}
              </div>
            </div>

            {/* Right Column: Chat/Turns List (2/3 Width) */}
            <div className="lg:col-span-2 flex flex-col h-full bg-[rgb(var(--background))] relative overflow-hidden rounded-3xl border border-[rgba(var(--border),0.05)] shadow-sm">
              {!selectedSession ? (
                <div className="flex-1 flex items-center justify-center text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-60">
                  Select a session to view conversation
                </div>
              ) : loading ? (
                <div className="flex-1 flex items-center justify-center">
                  <div className="w-6 h-6 border-2 border-[rgb(var(--accent))]/20 border-t-[rgb(var(--accent))] rounded-full animate-spin" />
                </div>
              ) : (
                <div className="flex-1 overflow-y-auto custom-scrollbar p-6 md:p-10">
                  <div className="max-w-5xl mx-auto space-y-8 pb-12">
                    
                    <div className="text-center pb-8 border-b border-[rgba(var(--border),0.05)] mb-8">
                      <h2 className="text-xl font-bold text-[rgb(var(--foreground))] mb-3">
                         {selectedSession.first_message || "Session Started"}
                      </h2>
                      <div className="inline-flex items-center gap-4 px-4 py-2 rounded-full bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.05)] text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-80">
                        <div className="flex items-center gap-1.5">
                          <Clock size={12} />
                          {formatDate(selectedSession.started_at)}
                        </div>
                        {selectedSession.ended_at && (
                           <>
                             <div className="w-1 h-1 rounded-full bg-[rgb(var(--foreground-muted))]/30" />
                             <div className="flex items-center gap-1.5">
                               Duration: {((selectedSession.ended_at - selectedSession.started_at) / 1000).toFixed(1)}s
                             </div>
                           </>
                        )}
                      </div>
                    </div>

                    <div className="space-y-6">
                      {turns.map((turn) => (
                        <div key={turn.id} className="space-y-6">
                          {/* User Message */}
                          <div className="flex flex-col items-end gap-2">
                            <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))] opacity-60">
                              User • {formatTime(turn.created_at)}
                            </span>
                            <div className="max-w-[85%] px-5 py-3.5 rounded-2xl rounded-tr-sm bg-[rgb(var(--accent))]/10 text-[rgb(var(--foreground))] border border-[rgb(var(--accent))]/20 shadow-[0_4px_24px_-4px_rgba(var(--accent),0.05)] text-sm md:text-base leading-relaxed">
                              {turn.user_text}
                            </div>
                          </div>

                          {/* Assistant Message */}
                          <div className="flex flex-col items-start gap-2">
                            <span className="flex items-center gap-2 text-[10px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))] opacity-80">
                              <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgb(var(--accent))]" />
                              Vox
                            </span>
                            <div className="max-w-[85%] px-5 py-3.5 rounded-2xl rounded-tl-sm bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] text-[rgb(var(--foreground-muted))] text-sm md:text-base leading-relaxed whitespace-pre-wrap">
                              {turn.assistant_text}
                            </div>
                            {/* Turn Metadata */}
                            <div className="flex items-center gap-4 text-[10px] font-mono tracking-wider text-[rgb(var(--foreground-muted))] opacity-60 pl-2">
                              {turn.stt_latency_ms !== null && (
                                <span>STT: {turn.stt_latency_ms}ms</span>
                              )}
                              {turn.ttft_ms !== null && (
                                <span>TTFT: {turn.ttft_ms}ms</span>
                              )}
                            </div>
                          </div>
                        </div>
                      ))}
                      {turns.length === 0 && (
                        <div className="text-center p-8 text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))] opacity-40">
                          No conversation data
                        </div>
                      )}
                    </div>

                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
