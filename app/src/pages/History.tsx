import React, {
  useState,
  useEffect,
  useCallback,
  useRef,
  useMemo,
  memo,
} from "react";
import { Ghost, ChevronLeft, ChevronRight, X, Trash2, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "@/shared/context/SettingsContext";
import { AnimatePresence, motion } from "framer-motion";

// ─── Types ────────────────────────────────────────────────────────────────────

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

interface HoverCardState {
  session: SessionRow;
  nodeRect: DOMRect;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const MAX_NODES = 24;

// ─── Helpers ─────────────────────────────────────────────────────────────────

function formatDateShort(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDateTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Recency opacity — newest = 1.0, oldest = 0.35, linear interpolation */
function recencyOpacity(index: number, total: number): number {
  if (total <= 1) return 1;
  return 1 - (index / (total - 1)) * 0.65;
}

// ─── Hover Preview Card ───────────────────────────────────────────────────────

interface HoverCardProps {
  session: SessionRow;
  nodeRect: DOMRect;
}

const HoverCard = memo(({ session, nodeRect }: HoverCardProps) => {
  const cardWidth = 220;
  const cardHeight = 120; // rough
  const viewportW = window.innerWidth;
  const viewportH = window.innerHeight;

  // Position adjacent to node, prefer right then left, prefer above then below
  let left = nodeRect.right + 12;
  if (left + cardWidth > viewportW - 16) {
    left = nodeRect.left - cardWidth - 12;
  }
  let top = nodeRect.top;
  if (top + cardHeight > viewportH - 80) {
    top = viewportH - 80 - cardHeight;
  }

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.92 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.92 }}
      transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
      className="fixed z-[300] w-[220px] glass-elevated glass-base rounded-2xl p-4 shadow-[0_16px_40px_rgba(0,0,0,0.5)] pointer-events-none"
      style={{ left, top }}
    >
      <div className="flex items-center justify-between mb-2">
        <span className="text-[9px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/70">
          Session #{session.id}
        </span>
        <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/50">
          {session.turn_count} turn{session.turn_count !== 1 ? "s" : ""}
        </span>
      </div>
      <p className="text-[12px] text-[rgb(var(--foreground))]/80 leading-relaxed line-clamp-3 font-light">
        {session.first_message || "No transcript recorded"}
      </p>
      <div className="mt-3 pt-2 border-t border-[rgba(var(--accent),0.08)]">
        <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40">
          {formatDateTime(session.started_at)}
        </span>
      </div>
    </motion.div>
  );
});
HoverCard.displayName = "HoverCard";

// ─── Session Node ─────────────────────────────────────────────────────────────

interface SessionNodeProps {
  session: SessionRow;
  index: number;
  total: number;
  isSelected: boolean;
  confirmDeleteId: number | null;
  onSelect: (session: SessionRow) => void;
  onHoverEnter: (session: SessionRow, rect: DOMRect) => void;
  onHoverLeave: () => void;
  onDelete: (e: React.MouseEvent, id: number) => void;
  onCancelDelete: (e: React.MouseEvent) => void;
}

const SessionNode = memo(
  ({
    session,
    index,
    total,
    isSelected,
    confirmDeleteId,
    onSelect,
    onHoverEnter,
    onHoverLeave,
    onDelete,
    onCancelDelete,
  }: SessionNodeProps) => {
    const nodeRef = useRef<HTMLButtonElement>(null);
    const opacity = recencyOpacity(index, total);
    const isConfirmingDelete = confirmDeleteId === session.id;

    const handleMouseEnter = useCallback(() => {
      if (nodeRef.current) {
        onHoverEnter(session, nodeRef.current.getBoundingClientRect());
      }
    }, [session, onHoverEnter]);

    return (
      <div
        className="relative flex flex-col items-center gap-2 group"
        style={{ opacity, transition: "opacity 0.5s ease" }}
      >
        {/* The node circle */}
        <button
          ref={nodeRef}
          onClick={() => onSelect(session)}
          onMouseEnter={handleMouseEnter}
          onMouseLeave={onHoverLeave}
          className={cn(
            "relative w-10 h-10 rounded-full transition-all duration-400 border flex items-center justify-center",
            isSelected
              ? "bg-[rgba(var(--accent),0.18)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.35),0_0_0_4px_rgba(var(--accent),0.08)] scale-110"
              : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--accent),0.18)] hover:border-[rgba(var(--accent),0.5)] hover:bg-[rgba(var(--accent),0.08)] hover:shadow-[0_0_16px_rgba(var(--accent),0.2)] hover:scale-105"
          )}
          aria-label={`Session ${session.id}: ${session.first_message ?? "No transcript"}`}
        >
          {/* Inner dot — recency indicator */}
          <span
            className={cn(
              "w-2 h-2 rounded-full transition-all duration-400",
              isSelected
                ? "bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)]"
                : "bg-[rgba(var(--accent),0.4)]"
            )}
          />
        </button>

        {/* Date label below node */}
        <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40 leading-none select-none">
          {formatDateShort(session.started_at)}
        </span>

        {/* Delete button — appears on group hover, anchored to top-right of node */}
        <div className="absolute -top-1 -right-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200">
          {isConfirmingDelete ? (
            <div className="flex items-center gap-0.5">
              <button
                onClick={(e) => onDelete(e, session.id)}
                className="w-5 h-5 rounded-full bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40 flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/30"
                aria-label="Confirm delete"
              >
                <Check size={9} strokeWidth={3} />
              </button>
              <button
                onClick={onCancelDelete}
                className="w-5 h-5 rounded-full bg-[rgba(var(--foreground),0.08)] border border-[rgba(var(--border),0.1)] flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.15)]"
                aria-label="Cancel delete"
              >
                <X size={9} strokeWidth={3} />
              </button>
            </div>
          ) : (
            <button
              onClick={(e) => onDelete(e, session.id)}
              className="w-5 h-5 rounded-full bg-[rgba(var(--foreground),0.08)] border border-[rgba(var(--border),0.1)] flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors"
              aria-label="Delete session"
            >
              <Trash2 size={9} />
            </button>
          )}
        </div>
      </div>
    );
  }
);
SessionNode.displayName = "SessionNode";

// ─── Detail Panel ─────────────────────────────────────────────────────────────

interface DetailPanelProps {
  session: SessionRow;
  sessions: SessionRow[];
  turns: TurnRow[];
  loading: boolean;
  onNavigate: (session: SessionRow) => void;
  onClose: () => void;
}

const DetailPanel = memo(
  ({ session, sessions, turns, loading, onNavigate, onClose }: DetailPanelProps) => {
    const currentIdx = sessions.findIndex((s) => s.id === session.id);
    const prevSession = currentIdx > 0 ? sessions[currentIdx - 1] : null;
    const nextSession = currentIdx < sessions.length - 1 ? sessions[currentIdx + 1] : null;

    return (
      <motion.div
        initial={{ y: "100%" }}
        animate={{ y: 0 }}
        exit={{ y: "100%" }}
        transition={{ duration: 0.38, ease: [0.16, 1, 0.3, 1] }}
        className="absolute bottom-0 left-0 right-0 h-[58%] glass-surface glass-base border-t border-[rgba(var(--accent),0.12)] z-30 flex flex-col shadow-[0_-20px_60px_rgba(0,0,0,0.4)] rounded-t-3xl overflow-hidden"
      >
        {/* Panel header */}
        <div className="flex items-center justify-between px-6 pt-5 pb-4 border-b border-[rgba(var(--accent),0.06)] shrink-0">
          <div className="flex items-center gap-3">
            {/* Prev / Next arrows */}
            <button
              onClick={() => prevSession && onNavigate(prevSession)}
              disabled={!prevSession}
              className="flex items-center justify-center w-7 h-7 rounded-full border border-[rgba(var(--accent),0.15)] bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.4)] disabled:opacity-20 disabled:cursor-not-allowed transition-all duration-200"
              aria-label="Previous session"
            >
              <ChevronLeft size={14} />
            </button>
            <button
              onClick={() => nextSession && onNavigate(nextSession)}
              disabled={!nextSession}
              className="flex items-center justify-center w-7 h-7 rounded-full border border-[rgba(var(--accent),0.15)] bg-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.4)] disabled:opacity-20 disabled:cursor-not-allowed transition-all duration-200"
              aria-label="Next session"
            >
              <ChevronRight size={14} />
            </button>

            <div>
              <span className="text-[11px] font-bold tracking-[0.2em] uppercase text-[rgb(var(--accent))]/80">
                Session #{session.id}
              </span>
              <div className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40 mt-0.5">
                {formatDateTime(session.started_at)} · {session.turn_count} turns
              </div>
            </div>
          </div>

          <button
            onClick={onClose}
            className="flex items-center justify-center w-7 h-7 rounded-full border border-[rgba(var(--accent),0.12)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
            aria-label="Close session"
          >
            <X size={14} />
          </button>
        </div>

        {/* Turn list */}
        <div className="flex-1 overflow-y-auto custom-scrollbar px-6 py-4">
          {loading ? (
            <div className="flex justify-center py-12">
              <div className="w-5 h-5 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
            </div>
          ) : turns.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 gap-2 opacity-50">
              <Ghost size={22} />
              <span className="text-[10px] font-bold uppercase tracking-widest">
                No interaction data
              </span>
            </div>
          ) : (
            <div className="space-y-5 pb-4">
              {turns.map((turn) => (
                <div
                  key={turn.id}
                  className="space-y-2 pb-5 border-b border-[rgba(var(--accent),0.04)] last:border-0 last:pb-0"
                >
                  {/* User utterance */}
                  <div className="flex items-start gap-2.5">
                    <span className="text-[9px] font-mono font-bold text-[rgb(var(--foreground-muted))]/35 uppercase tracking-widest shrink-0 mt-1">
                      you
                    </span>
                    <p className="text-[13px] font-light text-[rgb(var(--foreground))]/75 leading-relaxed">
                      {turn.user_text}
                    </p>
                  </div>

                  {/* Assistant response */}
                  <div className="flex items-start gap-2.5">
                    <span className="text-[9px] font-mono font-bold text-[rgb(var(--accent))]/60 uppercase tracking-widest shrink-0 mt-1">
                      vox
                    </span>
                    <div className="flex-1 min-w-0">
                      <p className="text-[13px] font-normal text-[rgb(var(--foreground))] leading-relaxed">
                        {turn.assistant_text}
                      </p>
                      <div className="flex gap-3 mt-1.5">
                        {turn.stt_latency_ms !== null && (
                          <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/25">
                            STT {turn.stt_latency_ms}ms
                          </span>
                        )}
                        {turn.ttft_ms !== null && (
                          <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/25">
                            TTFT {turn.ttft_ms}ms
                          </span>
                        )}
                        <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/25 ml-auto">
                          {formatTime(turn.created_at)}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </motion.div>
    );
  }
);
DetailPanel.displayName = "DetailPanel";

// ─── Main Component ───────────────────────────────────────────────────────────

export const History: React.FC = () => {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [sessionsLoading, setSessionsLoading] = useState(true);
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [turnsLoading, setTurnsLoading] = useState(false);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const [hoverCard, setHoverCard] = useState<HoverCardState | null>(null);
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { draftSettings, updateDraft } = useSettings();

  // Cap display to MAX_NODES, newest first
  const displaySessions = useMemo(
    () => sessions.slice(0, MAX_NODES),
    [sessions]
  );

  // Fetch sessions
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
      } catch (e) {
        console.error("Failed to fetch sessions on init:", e);
      } finally {
        setSessionsLoading(false);
      }
    };
    init();
  }, []);

  // Fetch turns for selected session
  useEffect(() => {
    if (!selectedSession) {
      setTurns([]);
      return;
    }
    const fetchTurns = async () => {
      setTurnsLoading(true);
      try {
        const data = await invoke<TurnRow[]>("get_turns", { sessionId: selectedSession.id });
        setTurns(data);
      } catch (e) {
        console.error("Failed to fetch turns:", e);
      } finally {
        setTurnsLoading(false);
      }
    };
    fetchTurns();
  }, [selectedSession]);

  // Delete handler
  const handleDelete = useCallback(
    async (e: React.MouseEvent, id: number) => {
      e.stopPropagation();
      if (confirmDeleteId === id) {
        try {
          await invoke("delete_session", { id });
          setConfirmDeleteId(null);
          if (selectedSession?.id === id) setSelectedSession(null);
          fetchSessions();
        } catch (e) {
          console.error("Failed to delete session:", e);
        }
      } else {
        setConfirmDeleteId(id);
        setTimeout(() => {
          setConfirmDeleteId((curr) => (curr === id ? null : curr));
        }, 3000);
      }
    },
    [confirmDeleteId, selectedSession, fetchSessions]
  );

  const handleCancelDelete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setConfirmDeleteId(null);
  }, []);

  // Hover card handlers — 120ms delay so quick mouse passes don't flash a card
  const handleHoverEnter = useCallback((session: SessionRow, rect: DOMRect) => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    hoverTimerRef.current = setTimeout(() => {
      setHoverCard({ session, nodeRect: rect });
    }, 120);
  }, []);

  const handleHoverLeave = useCallback(() => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    setHoverCard(null);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    };
  }, []);

  return (
    <div className="flex-1 flex flex-col h-full relative overflow-hidden bg-transparent select-none">

      {/* ── Top strip ───────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between px-8 pt-6 pb-4 shrink-0 z-20">
        <div>
          <span className="signal-text text-[13px]">Memory Recall</span>
          <p className="text-[10px] text-[rgb(var(--foreground-muted))]/50 font-mono uppercase tracking-[0.2em] mt-0.5">
            {sessions.length} session{sessions.length !== 1 ? "s" : ""} archived
          </p>
        </div>

        {/* Private mode toggle */}
        <div className="flex items-center gap-2.5">
          <span className="text-[10px] font-mono tracking-widest text-[rgb(var(--foreground-muted))]/50 uppercase">
            Private
          </span>
          <button
            onClick={() =>
              updateDraft("persistence", "private_mode", !draftSettings?.persistence.private_mode)
            }
            className={cn(
              "group relative flex items-center h-5 w-9 px-0.5 rounded-full transition-all duration-300",
              draftSettings?.persistence.private_mode
                ? "bg-[rgb(var(--accent))]"
                : "bg-black/35 border border-[rgba(var(--accent),0.2)]"
            )}
            aria-label={
              draftSettings?.persistence.private_mode ? "Private Mode Active" : "Enable Private Mode"
            }
            role="switch"
            aria-checked={draftSettings?.persistence.private_mode}
          >
            <div
              className={cn(
                "w-4 h-4 rounded-full bg-white transition-transform duration-300",
                draftSettings?.persistence.private_mode ? "translate-x-4" : "translate-x-0"
              )}
            />
          </button>
        </div>
      </div>

      {/* ── Node constellation grid ──────────────────────────────────────── */}
      <div className="flex-1 flex items-center justify-center px-8 z-20 min-h-0">
        {sessionsLoading ? (
          <div className="flex flex-col items-center gap-3 opacity-50">
            <div className="w-6 h-6 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
            <span className="text-[10px] font-bold uppercase tracking-widest">Loading memories...</span>
          </div>
        ) : displaySessions.length === 0 ? (
          <div className="flex flex-col items-center gap-3 opacity-40">
            <Ghost size={28} className="text-[rgb(var(--accent))]" />
            <span className="text-[11px] font-bold uppercase tracking-widest">No memories persisted</span>
            <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
              Voice sessions will appear here
            </span>
          </div>
        ) : (
          <div
            className="grid gap-x-8 gap-y-6"
            style={{
              // Adaptive columns: 6 max, shrink if fewer sessions
              gridTemplateColumns: `repeat(${Math.min(6, Math.ceil(Math.sqrt(displaySessions.length * 1.5)))}, minmax(0, 1fr))`,
            }}
          >
            {displaySessions.map((session, index) => (
              <SessionNode
                key={session.id}
                session={session}
                index={index}
                total={displaySessions.length}
                isSelected={selectedSession?.id === session.id}
                confirmDeleteId={confirmDeleteId}
                onSelect={setSelectedSession}
                onHoverEnter={handleHoverEnter}
                onHoverLeave={handleHoverLeave}
                onDelete={handleDelete}
                onCancelDelete={handleCancelDelete}
              />
            ))}
          </div>
        )}
      </div>

      {/* Hover preview card (portal-like fixed positioning) */}
      <AnimatePresence>
        {hoverCard && (
          <HoverCard
            key={hoverCard.session.id}
            session={hoverCard.session}
            nodeRect={hoverCard.nodeRect}
          />
        )}
      </AnimatePresence>

      {/* Detail panel slides up from bottom */}
      <AnimatePresence>
        {selectedSession && (
          <DetailPanel
            session={selectedSession}
            sessions={displaySessions}
            turns={turns}
            loading={turnsLoading}
            onNavigate={setSelectedSession}
            onClose={() => setSelectedSession(null)}
          />
        )}
      </AnimatePresence>
    </div>
  );
};
