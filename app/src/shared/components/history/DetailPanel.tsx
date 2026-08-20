import { memo, useState, useEffect, useRef } from "react";
import { motion } from "framer-motion";
import { Ghost, X, AlertCircle, RotateCcw } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { formatDateTime, type SessionRow, type TurnRow } from "@/services/historyService";
import { EmptyState, OrbitalLoader } from "@/shared/components/common";
import { cn } from "@/shared/lib/utils";
import { HISTORY_COPY } from "@/data/historyCopy";
import { Tooltip } from "@/shared/ui/Tooltip";

function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export interface DetailPanelProps {
  session: SessionRow;
  turns: TurnRow[];
  loading: boolean;
  error?: string | null;
  onClose: () => void;
  onRetry?: () => void;
}

const INITIAL_VISIBLE_TURNS = 20;
const DEFAULT_HEIGHT_PERCENT = 62;
const MIN_HEIGHT_PERCENT = 35;
const MAX_HEIGHT_PERCENT = 85;

export const DetailPanel = memo(
  ({ session, turns, loading, error, onClose, onRetry }: DetailPanelProps) => {
    const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_TURNS);
    const [heightPercent, setHeightPercent] = useState(DEFAULT_HEIGHT_PERCENT);
    const [isDragging, setIsDragging] = useState(false);
    const panelRef = useRef<HTMLDivElement>(null);
    const currentHeightRef = useRef(heightPercent);
    currentHeightRef.current = heightPercent;

    useEffect(() => {
      setVisibleCount(INITIAL_VISIBLE_TURNS);
    }, [session.id]);

    const visibleTurns = turns.slice(0, visibleCount);
    const hasMoreTurns = turns.length > visibleCount;

    // Handle vertical resizing drag with imperative height updates to prevent Markdown re-renders
    const handleDragStart = (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      const target = e.currentTarget;
      target.setPointerCapture(e.pointerId);

      setIsDragging(true);
      const startY = e.clientY;
      const startHeight = currentHeightRef.current;
      let lastHeight = startHeight;

      const onPointerMove = (moveEvent: PointerEvent) => {
        const deltaPx = startY - moveEvent.clientY;
        const deltaPercent = (deltaPx / window.innerHeight) * 100;
        const nextHeight = Math.min(
          MAX_HEIGHT_PERCENT,
          Math.max(MIN_HEIGHT_PERCENT, startHeight + deltaPercent)
        );
        lastHeight = nextHeight;
        if (panelRef.current) {
          panelRef.current.style.height = `${nextHeight}%`;
        }
      };

      const onPointerUp = (upEvent: PointerEvent) => {
        upEvent.preventDefault();
        upEvent.stopPropagation();
        setIsDragging(false);
        setHeightPercent(lastHeight);
        try {
          target.releasePointerCapture(upEvent.pointerId);
        } catch {}
        target.removeEventListener("pointermove", onPointerMove);
        target.removeEventListener("pointerup", onPointerUp);
        target.removeEventListener("pointercancel", onPointerUp);
      };

      target.addEventListener("pointermove", onPointerMove);
      target.addEventListener("pointerup", onPointerUp);
      target.addEventListener("pointercancel", onPointerUp);
    };

    // Double-click toggle between default & full expand
    const handleToggleExpand = (e: React.MouseEvent) => {
      e.stopPropagation();
      setHeightPercent((prev) =>
        prev > 70 ? DEFAULT_HEIGHT_PERCENT : MAX_HEIGHT_PERCENT
      );
    };

    return (
      <motion.div
        ref={panelRef}
        initial={{ y: "100%" }}
        animate={{ y: 0 }}
        exit={{ y: "100%" }}
        transition={{ duration: 0.38, ease: [0.16, 1, 0.3, 1] }}
        style={{ height: `${heightPercent}%` }}
        className={cn(
          "absolute bottom-0 left-0 right-0 z-30 flex flex-col rounded-t-3xl overflow-hidden glass-elevated border-0 shadow-2xl",
          isDragging ? "select-none transition-none" : "transition-[height] duration-150 ease-out"
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Top Vertical Drag Handle Bar */}
        <Tooltip label={HISTORY_COPY.resizeHint} wrapperClassName="w-full shrink-0">
          <div
            onPointerDown={handleDragStart}
            onDoubleClick={handleToggleExpand}
            className="w-full h-5 flex items-center justify-center cursor-row-resize group hover:bg-[rgb(var(--accent))]/5 transition-colors touch-none"
          >
            <div className="w-12 h-1 rounded-full bg-[rgba(var(--accent),0.3)] group-hover:bg-[rgb(var(--accent))] transition-colors shadow-sm" />
          </div>
        </Tooltip>

        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-1 pb-3 border-b border-[rgba(var(--accent),0.08)] shrink-0">
          <div>
            <span className="text-[13px] font-display font-black tracking-[0.16em] uppercase text-[rgb(var(--accent))]">
              {HISTORY_COPY.sessionPrefix}{session.id}
            </span>
            <div className="text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))] mt-0.5">
              {formatDateTime(session.started_at)} · {session.turn_count}{" "}
              {session.turn_count === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
            </div>
          </div>

          <button
            onClick={onClose}
            className="flex items-center justify-center w-8 h-8 rounded-full glass-card text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
            aria-label={HISTORY_COPY.closeSession}
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto overscroll-contain px-6 py-4 min-h-0 custom-scrollbar">
          {loading ? (
            <div className="flex justify-center py-12">
              <OrbitalLoader
                size="sm"
                title="Loading conversation transcript..."
                subtitle="Fetching turns and voice telemetry"
              />
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center py-12 px-4 text-center gap-3">
              <AlertCircle className="text-red-400 shrink-0" size={24} />
              <p className="text-[12px] text-red-400 font-medium max-w-xs leading-relaxed">{error}</p>
              {onRetry && (
                <button
                  onClick={onRetry}
                  className="px-3 py-1.5 rounded-xl glass-card border border-[rgba(var(--accent),0.3)] text-[11px] font-bold text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors flex items-center gap-1.5 cursor-pointer mt-1"
                >
                  <RotateCcw size={12} />
                  {HISTORY_COPY.retry}
                </button>
              )}
            </div>
          ) : turns.length === 0 ? (
            <EmptyState
              icon={Ghost}
              title={HISTORY_COPY.noConversationData}
              className="py-12 border-0 bg-transparent"
            />
          ) : (
            <div className="space-y-6 pb-4">
              {visibleTurns.map((turn) => (
                <div key={turn.id} className="space-y-4 [contain:content]">
                  {/* User bubble */}
                  <div className="flex flex-col items-end w-full">
                    <span className="text-[11px] font-sans font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 mr-2">
                      {HISTORY_COPY.userLabel}
                    </span>
                    <div className="glass-card rounded-2xl rounded-tr-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text">
                      <ReactMarkdown>{turn.user_text}</ReactMarkdown>
                    </div>
                  </div>

                  {/* Assistant bubble */}
                  <div className="flex flex-col items-start w-full">
                    <span className="text-[11px] font-sans font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-1 ml-2">
                      {HISTORY_COPY.voxLabel}
                    </span>
                    <div className="glass-card rounded-2xl rounded-tl-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text border border-[rgba(var(--accent),0.15)]">
                      <ReactMarkdown>{turn.assistant_text}</ReactMarkdown>
                      <div className="flex gap-3 mt-2 border-t border-[rgba(var(--accent),0.1)] pt-1.5 shrink-0 text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                        {turn.stt_latency_ms !== null && (
                          <Tooltip label={HISTORY_COPY.hearingTime}>
                            <span className="cursor-help hover:text-[rgb(var(--accent))] transition-colors">
                              {HISTORY_COPY.hearingPrefix} {turn.stt_latency_ms}ms
                            </span>
                          </Tooltip>
                        )}
                        {turn.ttft_ms !== null && (
                          <Tooltip label={HISTORY_COPY.thinkingTime}>
                            <span className="cursor-help hover:text-[rgb(var(--accent))] transition-colors">
                              {HISTORY_COPY.thinkingPrefix} {turn.ttft_ms}ms
                            </span>
                          </Tooltip>
                        )}
                        <span className="ml-auto text-[rgb(var(--foreground-muted))] font-medium">
                          {formatTime(turn.created_at)}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}

              {hasMoreTurns && (
                <div className="flex justify-center pt-2">
                  <button
                    onClick={() => setVisibleCount((prev) => prev + 20)}
                    className="px-4 py-2 rounded-xl glass-card text-[11px] font-bold text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer"
                  >
                    {HISTORY_COPY.loadOlderTurns} ({turns.length - visibleCount} {HISTORY_COPY.remaining})
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      </motion.div>
    );
  }
);

DetailPanel.displayName = "DetailPanel";

