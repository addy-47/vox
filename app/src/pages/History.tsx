import React, {
  useState,
  useEffect,
  useCallback,
  useRef,
  useMemo,
} from "react";
import { Ghost, ChevronLeft, ChevronRight, X, AlertCircle, RotateCcw, Check, Trash2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { AnimatePresence, motion } from "framer-motion";
import {
  getSessions,
  getTurns,
  deleteSession,
  formatDateShort,
  formatDateTime,
  type SessionRow,
  type TurnRow,
} from "@/services/historyService";
import { VoiceRippleNode } from "@/shared/components/history";
import { DetailPanel } from "@/shared/components/history";
import { EmptyState } from "@/shared/components/common/EmptyState";
import { HISTORY_COPY } from "@/data/historyCopy";

function getErrorMessage(e: unknown, fallback: string): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e && typeof (e as { message: unknown }).message === "string") {
    return (e as { message: string }).message;
  }
  return fallback;
}

// ─── Main Component ───────────────────────────────────────────────────────────

export const History: React.FC = () => {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [sessionsLoading, setSessionsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [turnsLoading, setTurnsLoading] = useState(false);
  const [turnsError, setTurnsError] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const deleteTimerRef = useRef<NodeJS.Timeout | null>(null);

  // Pagination & Layout states
  const [pageIndex, setPageIndex] = useState(0);
  const [pageDirection, setPageDirection] = useState(1);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  useEffect(() => {
    if (!containerRef.current) return;
    let timer: NodeJS.Timeout;
    const observer = new ResizeObserver((entries) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        for (const entry of entries) {
          setDimensions({
            width: entry.contentRect.width,
            height: entry.contentRect.height,
          });
        }
      }, 120);
    });
    observer.observe(containerRef.current);
    return () => {
      observer.disconnect();
      clearTimeout(timer);
    };
  }, []);

  const fetchSessions = useCallback(async () => {
    setError(null);
    try {
      const data = await getSessions();
      setSessions(data.sort((a, b) => b.started_at - a.started_at));
    } catch (e: unknown) {
      console.error("Failed to fetch sessions:", e);
      setError(getErrorMessage(e, HISTORY_COPY.failedFallback));
    }
  }, []);

  const loadSessions = useCallback(async () => {
    setSessionsLoading(true);
    await fetchSessions();
    setSessionsLoading(false);
  }, [fetchSessions]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  useEffect(() => {
    if (!selectedSession) {
      setTurns([]);
      setTurnsError(null);
      return;
    }
    let isCancelled = false;
    const fetchTurns = async () => {
      setTurnsLoading(true);
      setTurnsError(null);
      try {
        const data = await getTurns(selectedSession.id);
        if (!isCancelled) {
          setTurns(data);
        }
      } catch (e: unknown) {
        console.error("Failed to fetch turns:", e);
        if (!isCancelled) {
          setTurnsError(getErrorMessage(e, "Failed to load session transcript turns."));
        }
      } finally {
        if (!isCancelled) {
          setTurnsLoading(false);
        }
      }
    };
    fetchTurns();

    return () => {
      isCancelled = true;
    };
  }, [selectedSession]);

  const retryFetchTurns = useCallback(() => {
    if (!selectedSession) return;
    setTurnsLoading(true);
    setTurnsError(null);
    getTurns(selectedSession.id)
      .then((data) => {
        setTurns(data);
      })
      .catch((e: unknown) => {
        console.error("Failed to fetch turns:", e);
        setTurnsError(getErrorMessage(e, "Failed to load session transcript turns."));
      })
      .finally(() => {
        setTurnsLoading(false);
      });
  }, [selectedSession]);

  const handleDelete = useCallback(
    async (e: React.MouseEvent, id: number) => {
      e.stopPropagation();
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);

      if (confirmDeleteId === id) {
        try {
          await deleteSession(id);
          setConfirmDeleteId(null);
          setDeleteError(null);
          if (selectedSession?.id === id) setSelectedSession(null);
          fetchSessions();
        } catch (err: unknown) {
          console.error("Failed to delete session:", err);
          setDeleteError(getErrorMessage(err, HISTORY_COPY.deleteFailed));
        }
      } else {
        setConfirmDeleteId(id);
        deleteTimerRef.current = setTimeout(() => {
          setConfirmDeleteId(null);
        }, 3000);
      }
    },
    [confirmDeleteId, selectedSession, fetchSessions]
  );

  useEffect(() => {
    if (!deleteError) return;
    const timer = setTimeout(() => setDeleteError(null), 5000);
    return () => clearTimeout(timer);
  }, [deleteError]);

  const handleCancelDelete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    setConfirmDeleteId(null);
  }, []);

  const layoutParams = useMemo(() => {
    const paddingX = 56;
    const paddingY = 48;
    const usableW = Math.max(200, dimensions.width - paddingX * 2);
    const usableH = Math.max(200, dimensions.height - paddingY * 2);
    const cols = Math.max(2, Math.floor(usableW / 240));
    const rows = Math.max(2, Math.floor(usableH / 140));
    const maxPerPage = cols * rows;
    return { cols, rows, maxPerPage, paddingX, paddingY, usableW, usableH };
  }, [dimensions]);

  const { maxPerPage, paddingX, paddingY, usableW, usableH } = layoutParams;
  const totalPages = Math.max(1, Math.ceil(sessions.length / maxPerPage));

  useEffect(() => {
    if (pageIndex >= totalPages) setPageIndex(Math.max(0, totalPages - 1));
  }, [totalPages, pageIndex]);

  const pageSessions = useMemo(() => {
    return sessions.slice(pageIndex * maxPerPage, (pageIndex + 1) * maxPerPage);
  }, [sessions, pageIndex, maxPerPage]);

  // Deterministic Grid Layout with Seeded Radial Jitter (No main-thread D3-Force physics simulation)
  const nodesWithPositions = useMemo(() => {
    const N = pageSessions.length;
    if (N === 0 || dimensions.width === 0) return [];

    const seededRandom = (seed: number) => {
      const x = Math.sin(seed) * 10000;
      return x - Math.floor(x);
    };

    const isCompact = dimensions.width < 680;
    const cols = isCompact ? 1 : Math.max(2, Math.floor(usableW / 240));
    const rows = Math.max(2, Math.floor(usableH / 140));
    const totalSlots = cols * rows;

    if (isCompact) {
      return pageSessions.map((session, index) => {
        const cellH = usableH / N;
        const centerX = dimensions.width / 2;
        const centerY = paddingY + index * cellH + cellH / 2;
        return {
          session,
          x: centerX,
          y: centerY,
          dayKey: formatDateShort(session.started_at),
        };
      });
    }

    const availableSlots = Array.from({ length: Math.max(1, totalSlots - 2) }, (_, idx) => idx + 1);
    let seed = pageIndex + 42;
    const nextRand = () => {
      seed = (seed * 9301 + 49297) % 233280;
      return seed / 233280;
    };
    for (let i = availableSlots.length - 1; i > 0; i--) {
      const j = Math.floor(nextRand() * (i + 1));
      const temp = availableSlots[i];
      availableSlots[i] = availableSlots[j];
      availableSlots[j] = temp;
    }

    const edgeMarginX = 64;
    const edgeMarginY = 64;

    return pageSessions.map((session, index) => {
      let cellIdx: number;
      if (index === 0) {
        cellIdx = 0;
      } else if (index === N - 1 && N > 1) {
        cellIdx = totalSlots - 1;
      } else {
        const slotIdx = (index - 1) % availableSlots.length;
        cellIdx = availableSlots[slotIdx];
      }

      const colIdx = cellIdx % cols;
      const rowIdx = Math.floor(cellIdx / cols);
      const cellW = usableW / cols;
      const cellH = usableH / rows;

      const centerX = paddingX + colIdx * cellW + cellW / 2;
      const centerY = paddingY + rowIdx * cellH + cellH / 2;

      const seedJitter = session.id * 13;
      const rx = seededRandom(seedJitter);
      const ry = seededRandom(seedJitter + 7);

      const maxJitterX = Math.max(0, (cellW - 224) / 4);
      const maxJitterY = Math.max(0, (cellH - 112) / 4);
      const jitterX = (rx * 2 - 1) * maxJitterX;
      const jitterY = (ry * 2 - 1) * maxJitterY;

      const rawX = centerX + jitterX;
      const rawY = centerY + jitterY;

      const clampedX = Math.max(112 + edgeMarginX, Math.min(dimensions.width - 112 - edgeMarginX, rawX));
      const clampedY = Math.max(56 + edgeMarginY, Math.min(dimensions.height - 56 - edgeMarginY, rawY));

      return {
        session,
        x: clampedX,
        y: clampedY,
        dayKey: formatDateShort(session.started_at),
      };
    });
  }, [pageSessions, pageIndex, usableW, usableH, paddingX, paddingY, dimensions]);

  const dayPaths = useMemo(() => {
    const groups: { [day: string]: typeof nodesWithPositions } = {};
    nodesWithPositions.forEach((node) => {
      if (!groups[node.dayKey]) groups[node.dayKey] = [];
      groups[node.dayKey].push(node);
    });
    return Object.entries(groups)
      .map(([day, nodes]) => {
        if (nodes.length < 2) return null;
        const sorted = [...nodes].sort((a, b) => a.session.started_at - b.session.started_at);
        const d = sorted.map((node, i) => `${i === 0 ? "M" : "L"} ${node.x} ${node.y}`).join(" ");
        return { day, d };
      })
      .filter((p): p is { day: string; d: string } => p !== null);
  }, [nodesWithPositions]);

  const handlePrevPage = () => {
    if (pageIndex > 0) {
      setPageDirection(-1);
      setPageIndex((p) => p - 1);
    }
  };
  const handleNextPage = () => {
    if (pageIndex < totalPages - 1) {
      setPageDirection(1);
      setPageIndex((p) => p + 1);
    }
  };

  const isMeasuring = dimensions.width === 0;
  const showLoading = sessionsLoading || isMeasuring;

  return (
    <div className="flex-1 flex flex-col h-full relative overflow-hidden bg-transparent select-none">
      {/* Delete error notification feedback */}
      <AnimatePresence>
        {deleteError && (
          <motion.div
            initial={{ opacity: 0, y: -20, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -20, scale: 0.95 }}
            className="absolute top-4 left-1/2 -translate-x-1/2 z-[100] max-w-md w-full px-4 pointer-events-auto"
          >
            <div className="glass-card p-3.5 rounded-xl flex items-center justify-between gap-3 border border-red-500/30 shadow-2xl bg-[rgb(var(--card))]/90 backdrop-blur-md text-left">
              <div className="flex items-center gap-2.5 min-w-0">
                <AlertCircle className="text-red-400 shrink-0" size={16} />
                <span className="text-[12px] font-medium text-[rgb(var(--foreground))] truncate">
                  {deleteError}
                </span>
              </div>
              <button
                onClick={() => setDeleteError(null)}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-1 rounded-lg transition-colors cursor-pointer"
                aria-label="Dismiss error"
              >
                <X size={14} />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div
        ref={containerRef}
        className="flex-1 relative z-20 min-h-0 pt-6 flex flex-col"
        onClick={() => setSelectedSession(null)}
      >
        {showLoading ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 opacity-50">
            <div className="w-6 h-6 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
            <span className="text-[11px] font-bold uppercase tracking-widest">{HISTORY_COPY.loading}</span>
          </div>
        ) : error ? (
          <div className="absolute inset-0 flex items-center justify-center p-8">
            <div className="max-w-sm w-full glass-card p-6 rounded-2xl flex flex-col items-center text-center gap-4 border border-red-500/20">
              <div className="w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center text-red-400">
                <AlertCircle size={20} />
              </div>
              <div className="space-y-1">
                <h3 className="text-[14px] font-bold text-[rgb(var(--foreground))]">{HISTORY_COPY.failedTitle}</h3>
                <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed">
                  {error}
                </p>
              </div>
              <button
                onClick={loadSessions}
                className="px-4 py-2 rounded-xl glass-card border border-[rgba(var(--accent),0.3)] text-[12px] font-bold text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors flex items-center gap-2 cursor-pointer"
              >
                <RotateCcw size={14} />
                {HISTORY_COPY.retry}
              </button>
            </div>
          </div>
        ) : sessions.length === 0 ? (
          <div className="absolute inset-0 flex items-center justify-center p-8">
            <EmptyState
              icon={Ghost}
              title={HISTORY_COPY.noMemoriesTitle}
              description={HISTORY_COPY.noMemoriesDesc}
              className="max-w-sm border-0 bg-transparent"
            />
          </div>
        ) : dimensions.width < 680 ? (
          // ─── Compact mobile list view ───────────────────────────────────────
          <div className="flex-1 overflow-y-auto px-6 pb-6 space-y-4 custom-scrollbar">
            {sessions.map((session) => {
              const isSelected = selectedSession?.id === session.id;
              const isConfirmingDelete = confirmDeleteId === session.id;
              const previewText = session.first_message || HISTORY_COPY.noTranscript;
              return (
                <div
                  key={session.id}
                  onClick={(e) => {
                    e.stopPropagation();
                    setSelectedSession(session);
                  }}
                  className={cn(
                    "w-full rounded-2xl p-4 flex flex-col text-left transition-colors duration-300 select-none cursor-pointer relative group glass-card",
                    isSelected
                      ? "border-[rgba(var(--accent),0.6)] bg-[rgba(var(--accent),0.12)]"
                      : "hover:bg-[rgba(var(--accent),0.04)]"
                  )}
                >
                  <div className="flex items-center justify-between mb-2 pr-10">
                    <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">
                      {formatDateTime(session.started_at)}
                    </span>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] font-medium">
                      {session.turn_count} {session.turn_count === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
                    </span>
                  </div>
                  <p className="text-[14px] font-light leading-relaxed italic text-[rgb(var(--foreground))] pr-10">
                    "{previewText}"
                  </p>
                  <div className="absolute top-4 right-4 z-20">
                    {isConfirmingDelete ? (
                      <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                        <button
                          onClick={(e) => handleDelete(e, session.id)}
                          className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 cursor-pointer shadow-md"
                          aria-label="Confirm delete"
                        >
                          <Check size={14} strokeWidth={2.5} />
                        </button>
                        <button
                          onClick={handleCancelDelete}
                          className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shadow-md"
                          aria-label="Cancel delete"
                        >
                          <X size={14} strokeWidth={2.5} />
                        </button>
                      </div>
                    ) : (
                      <button
                        onClick={(e) => handleDelete(e, session.id)}
                        className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors opacity-0 group-hover:opacity-100 focus:opacity-100 cursor-pointer"
                        aria-label="Delete session"
                      >
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          // ─── Desktop constellation view ─────────────────────────────────────
          <>
            <button
              onClick={(e) => { e.stopPropagation(); handlePrevPage(); }}
              disabled={pageIndex === 0}
              className="absolute left-3 top-1/2 -translate-y-1/2 w-9 h-9 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] disabled:opacity-10 transition-all z-30 cursor-pointer"
              aria-label="Newer sessions page"
            >
              <ChevronLeft size={22} />
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); handleNextPage(); }}
              disabled={pageIndex === totalPages - 1}
              className="absolute right-3 top-1/2 -translate-y-1/2 w-9 h-9 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] disabled:opacity-10 transition-all z-30 cursor-pointer"
              aria-label="Older sessions page"
            >
              <ChevronRight size={22} />
            </button>

            <div className="absolute bottom-2 left-1/2 -translate-x-1/2 text-[10px] font-mono font-medium text-[rgb(var(--foreground-muted))] z-30">
              {pageIndex + 1} / {totalPages}
            </div>

            <div className="w-full h-full relative overflow-hidden">
              <AnimatePresence mode="popLayout" custom={pageDirection}>
                <motion.div
                  key={pageIndex}
                  custom={pageDirection}
                  initial={{ opacity: 0, x: pageDirection * 50 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -pageDirection * 50 }}
                  transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
                  className="w-full h-full absolute inset-0"
                >
                  <svg className="absolute inset-0 w-full h-full pointer-events-none z-0">
                    {dayPaths.map((path) => (
                      <path
                        key={path.day}
                        d={path.d}
                        fill="none"
                        stroke="rgba(var(--accent), 0.18)"
                        strokeWidth="1.5"
                        strokeDasharray="4 6"
                      />
                    ))}
                  </svg>
                  <AnimatePresence>
                    {nodesWithPositions.map(({ session, x, y }) => (
                      <VoiceRippleNode
                        key={session.id}
                        session={session}
                        isSelected={selectedSession?.id === session.id}
                        isConfirmingDelete={confirmDeleteId === session.id}
                        onSelect={setSelectedSession}
                        onDelete={handleDelete}
                        onCancelDelete={handleCancelDelete}
                        x={x}
                        y={y}
                      />
                    ))}
                  </AnimatePresence>
                </motion.div>
              </AnimatePresence>
            </div>
          </>
        )}
      </div>

      <AnimatePresence>
        {selectedSession && (
          <div
            className="absolute inset-0 z-[25] cursor-default glass-card bg-[rgb(var(--background))]/50 backdrop-blur-sm"
            onClick={() => setSelectedSession(null)}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {selectedSession && (
          <DetailPanel
            session={selectedSession}
            turns={turns}
            loading={turnsLoading}
            error={turnsError}
            onClose={() => setSelectedSession(null)}
            onRetry={retryFetchTurns}
          />
        )}
      </AnimatePresence>
    </div>
  );
};
