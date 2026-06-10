import React, { useState, useEffect, useCallback } from "react";
import { MessageSquare, Trash2, Check, X, History as HistoryIcon, Ghost } from "lucide-react";
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
  const [sessionsLoading, setSessionsLoading] = useState(true);
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const { draftSettings, updateDraft } = useSettings();

  const fetchSessions = useCallback(async () => {
    try {
      const data = await invoke<SessionRow[]>("get_sessions");
      setSessions(data);
    } catch (e) {
      console.error("Failed to fetch sessions:", e);
    }
  }, []);

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
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent p-6 md:p-10 select-none">
      
      {/* ── Header ──────────────────────────────────────────────────────────── */}
      <header className="flex flex-col md:flex-row md:items-end justify-between border-b border-[rgba(var(--accent),0.08)] pb-6 shrink-0 gap-4">
        <div className="space-y-1">
          <div className="flex items-center gap-3">
            <span className="signal-text text-[14px]">MEMORY recall</span>
          </div>
          <p className="text-[11px] text-[rgb(var(--foreground-muted))] font-light uppercase tracking-wider">
            Review and audit historical voice interaction logs.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--foreground-muted))]/60 uppercase">
            Private Mode
          </span>
          <button
            onClick={() => updateDraft("persistence", "private_mode", !(draftSettings?.persistence.private_mode))}
            className={cn(
              "group relative flex items-center h-6 w-11 px-0.5 rounded-full transition-all duration-300",
              draftSettings?.persistence.private_mode 
                ? "bg-[rgb(var(--accent))]" 
                : "bg-black/35 border border-[rgba(var(--accent),0.2)]"
            )}
            aria-label={draftSettings?.persistence.private_mode ? "Private Mode Active" : "Enable Private Mode"}
          >
            <div className={cn(
              "flex items-center justify-center w-5 h-5 rounded-full bg-white transition-all duration-300 transform",
              draftSettings?.persistence.private_mode ? "translate-x-5" : "translate-x-0"
            )}>
              <Ghost className={cn("w-3 h-3", draftSettings?.persistence.private_mode ? "text-[rgb(var(--accent))]" : "text-slate-400")} />
            </div>
          </button>
        </div>
      </header>

      {/* ── Timeline Strip (Top Horizontal Navigation) ──────────────────────── */}
      <div className="py-4 border-b border-[rgba(var(--accent),0.04)] shrink-0 overflow-x-auto scrollbar-none flex gap-3">
        {sessionsLoading ? (
          <div className="flex gap-3 animate-pulse">
            <div className="h-8 w-32 bg-[rgba(var(--accent),0.05)] rounded-full" />
            <div className="h-8 w-40 bg-[rgba(var(--accent),0.05)] rounded-full" />
            <div className="h-8 w-28 bg-[rgba(var(--accent),0.05)] rounded-full" />
          </div>
        ) : sessions.length === 0 ? (
          <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 uppercase tracking-widest">
            No memories persisted.
          </span>
        ) : (
          sessions.map(session => (
            <div
              key={session.id}
              onClick={() => setSelectedSession(session)}
              className={cn(
                "group shrink-0 relative flex items-center gap-2.5 px-4 py-1.5 rounded-full cursor-pointer transition-all duration-350 border text-[11px] font-mono tracking-wider",
                selectedSession?.id === session.id
                  ? "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.1)]"
                  : "border-[rgba(var(--accent),0.1)] bg-black/20 text-[rgb(var(--foreground-muted))] hover:border-[rgba(var(--accent),0.25)] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <MessageSquare size={12} className="shrink-0" />
              <span className="max-w-[140px] truncate">
                {session.first_message || "New Session"}
              </span>
              <span className="opacity-50 text-[9px]">{formatDate(session.started_at)}</span>

              {/* Delete button nested elegantly inside pill */}
              <div className="flex items-center shrink-0">
                {confirmDeleteId === session.id ? (
                  <div className="flex items-center gap-1 animate-in fade-in zoom-in duration-200">
                    <button
                      onClick={(e) => handleDelete(e, session.id)}
                      className="text-emerald-400 hover:scale-110 p-0.5"
                    >
                      <Check size={11} strokeWidth={3} />
                    </button>
                    <button
                      onClick={handleCancelDelete}
                      className="text-red-400 hover:scale-110 p-0.5"
                    >
                      <X size={11} strokeWidth={3} />
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={(e) => handleDelete(e, session.id)}
                    className="opacity-0 group-hover:opacity-100 hover:text-red-400 p-0.5 transition-opacity duration-200"
                    aria-label="Delete Session"
                  >
                    <Trash2 size={11} />
                  </button>
                )}
              </div>
            </div>
          ))
        )}
      </div>

      {/* ── Dialogue Timeline (Fullscreen scroll area) ────────────────────── */}
      <main className="flex-1 overflow-y-auto custom-scrollbar py-6">
        <div className="max-w-4xl mx-auto w-full">
          {!selectedSession ? (
            <div className="flex flex-col items-center justify-center py-24 gap-3 opacity-30 h-full">
              <HistoryIcon size={32} className="text-[rgb(var(--accent))]" />
              <span className="signal-text text-[10px]">SELECT A SESSION TO RECALL MEMORIES</span>
            </div>
          ) : loading ? (
            <div className="flex justify-center py-24">
              <div className="w-5 h-5 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
            </div>
          ) : (
            <div className="space-y-6 pb-16">
              
              {/* Session Meta Headers */}
              <div className="flex justify-between items-center text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 border-b border-[rgba(var(--accent),0.04)] pb-4 mb-8">
                <span>SESSION #{selectedSession.id}</span>
                <span>TOTAL TURNS: {selectedSession.turn_count}</span>
              </div>

              {turns.map((turn) => (
                <div 
                  key={turn.id} 
                  className="group flex flex-col gap-3 py-4 border-b border-[rgba(var(--accent),0.04)] last:border-0 hover:bg-[rgba(var(--accent),0.01)] rounded-lg px-2 transition-colors duration-300"
                >
                  
                  {/* User Entry */}
                  <div className="flex flex-col md:flex-row md:items-baseline justify-between gap-1">
                    <div className="flex items-start gap-2.5">
                      <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))]/40 uppercase tracking-widest shrink-0 mt-0.5">
                        [YOU]
                      </span>
                      <p className="text-[14px] md:text-[15px] font-light text-[rgb(var(--foreground))]/80 leading-relaxed">
                        {turn.user_text}
                      </p>
                    </div>
                    <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40 self-end md:self-auto shrink-0">
                      {formatTime(turn.created_at)}
                    </span>
                  </div>

                  {/* Assistant Entry */}
                  <div className="flex flex-col md:flex-row md:items-baseline justify-between gap-1 mt-1">
                    <div className="flex items-start gap-2.5">
                      <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]/70 uppercase tracking-widest shrink-0 mt-0.5">
                        [VOX]
                      </span>
                      <p className="text-[14px] md:text-[15px] font-normal text-[rgb(var(--foreground))] leading-relaxed">
                        {turn.assistant_text}
                      </p>
                    </div>

                    {/* Latencies on the right */}
                    <div className="flex gap-3 text-[8px] font-mono text-[rgb(var(--foreground-muted))]/30 uppercase self-end md:self-auto shrink-0">
                      {turn.stt_latency_ms !== null && <span>STT: {turn.stt_latency_ms}ms</span>}
                      {turn.ttft_ms !== null && <span>TTFT: {turn.ttft_ms}ms</span>}
                    </div>
                  </div>

                </div>
              ))}

              {turns.length === 0 && (
                <div className="flex flex-col items-center justify-center py-20 opacity-30">
                  <Ghost size={24} className="mb-2" />
                  <span className="signal-text text-[9px]">NO INTERACTION DATA FOUND</span>
                </div>
              )}

            </div>
          )}
        </div>
      </main>
    </div>
  );
};
