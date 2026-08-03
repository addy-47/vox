import { memo } from "react";
import { motion } from "framer-motion";
import { Ghost, X } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { type SessionRow, type TurnRow } from "@/services/historyService";

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

export interface DetailPanelProps {
  session: SessionRow;
  turns: TurnRow[];
  loading: boolean;
  onClose: () => void;
}

export const DetailPanel = memo(
  ({ session, turns, loading, onClose }: DetailPanelProps) => {
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
        <div className="flex items-center justify-between px-6 pt-5 pb-4 border-b border-[rgba(var(--accent),0.06)] shrink-0">
          <div>
            <span className="text-[11px] font-bold tracking-[0.2em] uppercase text-[rgb(var(--accent))]/80">
              Session #{session.id}
            </span>
            <div className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40 mt-0.5">
              {formatDateTime(session.started_at)} · {session.turn_count} turns
            </div>
          </div>

          <button
            onClick={onClose}
            className="flex items-center justify-center w-7 h-7 rounded-full glass text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
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
          ) : turns.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 gap-2 opacity-50">
              <Ghost size={28} />
              <span className="text-[10px] font-bold uppercase tracking-widest">
                No interaction data
              </span>
            </div>
          ) : (
            <div className="space-y-6 pb-4">
              {turns.map((turn) => (
                <div key={turn.id} className="space-y-4">
                  {/* User bubble */}
                  <div className="flex flex-col items-end w-full">
                    <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))]/40 uppercase tracking-widest mb-1 mr-2">
                      you
                    </span>
                    <div className="glass rounded-2xl rounded-tr-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))]/85 leading-relaxed break-words prose prose-invert select-text">
                      <ReactMarkdown>{turn.user_text}</ReactMarkdown>
                    </div>
                  </div>

                  {/* Assistant bubble */}
                  <div className="flex flex-col items-start w-full">
                    <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]/70 uppercase tracking-widest mb-1 ml-2">
                      vox
                    </span>
                    <div className="glass rounded-2xl rounded-tl-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text">
                      <ReactMarkdown>{turn.assistant_text}</ReactMarkdown>
                      <div className="flex gap-3 mt-2 border-t border-[rgba(var(--accent),0.06)] pt-1.5 shrink-0">
                        {turn.stt_latency_ms !== null && (
                          <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/30">
                            STT {turn.stt_latency_ms}ms
                          </span>
                        )}
                        {turn.ttft_ms !== null && (
                          <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/30">
                            TTFT {turn.ttft_ms}ms
                          </span>
                        )}
                        <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/30 ml-auto">
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
