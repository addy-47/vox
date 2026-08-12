import { memo, useState } from "react";
import { motion } from "framer-motion";
import { Ghost, X, AlertCircle, RotateCcw } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { formatDateTime, type SessionRow, type TurnRow } from "@/services/historyService";
import { EmptyState } from "@/shared/components/common/EmptyState";
import { HISTORY_COPY } from "@/data/historyCopy";

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

export const DetailPanel = memo(
  ({ session, turns, loading, error, onClose, onRetry }: DetailPanelProps) => {
    const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_TURNS);

    const visibleTurns = turns.slice(0, visibleCount);
    const hasMoreTurns = turns.length > visibleCount;

    return (
      <motion.div
        initial={{ y: "100%" }}
        animate={{ y: 0 }}
        exit={{ y: "100%" }}
        transition={{ duration: 0.38, ease: [0.16, 1, 0.3, 1] }}
        className="absolute bottom-0 left-0 right-0 h-[65%] md:h-[65%] z-30 flex flex-col rounded-t-3xl overflow-hidden glass-card border-0"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-5 pb-4 border-b border-[rgba(var(--accent),0.08)] shrink-0">
          <div>
            <span className="text-[12px] font-bold tracking-[0.2em] uppercase text-[rgb(var(--accent))]">
              Session #{session.id}
            </span>
            <div className="text-[10px] font-mono font-medium text-[rgb(var(--foreground-muted))] mt-0.5">
              {formatDateTime(session.started_at)} · {session.turn_count} {session.turn_count === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
            </div>
          </div>

          <button
            onClick={onClose}
            className="flex items-center justify-center w-8 h-8 rounded-full glass-card text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
            aria-label="Close session"
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto custom-scrollbar px-6 py-4 min-h-0">
          {loading ? (
            <div className="flex justify-center py-12">
              <div className="w-5 h-5 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
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
              title="No interaction data"
              className="py-12 border-0 bg-transparent"
            />
          ) : (
            <div className="space-y-6 pb-4">
              {visibleTurns.map((turn) => (
                <div key={turn.id} className="space-y-4">
                  {/* User bubble */}
                  <div className="flex flex-col items-end w-full">
                    <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 mr-2">
                      USER
                    </span>
                    <div className="glass-card rounded-2xl rounded-tr-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text">
                      <ReactMarkdown>{turn.user_text}</ReactMarkdown>
                    </div>
                  </div>

                  {/* Assistant bubble */}
                  <div className="flex flex-col items-start w-full">
                    <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-1 ml-2">
                      VOX
                    </span>
                    <div className="glass-card rounded-2xl rounded-tl-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text border border-[rgba(var(--accent),0.15)]">
                      <ReactMarkdown>{turn.assistant_text}</ReactMarkdown>
                      <div className="flex gap-3 mt-2 border-t border-[rgba(var(--accent),0.1)] pt-1.5 shrink-0 text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                        {turn.stt_latency_ms !== null && (
                          <span title="Speech-To-Text Audio Recognition Latency" className="hover:text-[rgb(var(--accent))] transition-colors">
                            STT {turn.stt_latency_ms}ms
                          </span>
                        )}
                        {turn.ttft_ms !== null && (
                          <span title="Time-To-First-Token Response Speed" className="hover:text-[rgb(var(--accent))] transition-colors">
                            TTFT {turn.ttft_ms}ms
                          </span>
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
                    className="px-4 py-2 rounded-xl glass-card text-[11px] font-mono font-bold text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer"
                  >
                    Load Older Turns ({turns.length - visibleCount} remaining)
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
