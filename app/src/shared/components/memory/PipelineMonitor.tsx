import React, { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Cpu, Play, X, RefreshCw } from "lucide-react";
import { MemoryQueueSummary, triggerMemoryConsolidation } from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

interface PipelineMonitorProps {
  summary: MemoryQueueSummary | null;
  onRefresh: () => void;
  open: boolean;
  onClose: () => void;
}

const STAGES = [
  { key: "staged_pending", label: "DEDUP" },
  { key: "dedup_pass", label: "EMBED" },
  { key: "nli_evaluated", label: "EVAL" },
  { key: "paused", label: "COMMIT" },
] as const;

export const PipelineMonitorPopover: React.FC<PipelineMonitorProps> = ({
  summary,
  onRefresh,
  open,
  onClose,
}) => {
  const [running, setRunning] = useState(false);

  const totalPending = summary
    ? (summary.staged_pending ?? 0) +
      (summary.dedup_pass ?? 0) +
      (summary.nli_evaluated ?? 0) +
      (summary.paused ?? 0)
    : 0;

  const handleTrigger = async () => {
    setRunning(true);
    try {
      await triggerMemoryConsolidation();
      setTimeout(onRefresh, 800);
    } catch (e) {
      console.error("Consolidation trigger failed:", e);
    } finally {
      setRunning(false);
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0, scale: 0.92, y: 8 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.92, y: 8 }}
          transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
          className="fixed z-[200] bottom-[68px] right-4 w-[260px] glass-card p-3.5 rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgba(10,12,14,0.85)] backdrop-blur-2xl shadow-2xl flex flex-col gap-3"
          role="dialog"
          aria-label="Pipeline Queue"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-white/[0.06] pb-2">
            <div className="flex items-center gap-2">
              <Cpu size={14} className="text-[rgb(var(--accent))]" />
              <span className="text-[10px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--foreground-muted))]/80 flex items-center gap-1.5">
                PIPELINE QUEUE
                {totalPending > 0 && (
                  <span className="text-[9px] font-mono font-bold text-[rgb(var(--accent))]">
                    ({totalPending})
                  </span>
                )}
              </span>
            </div>
            <div className="flex items-center gap-1">
              <button
                onClick={handleTrigger}
                disabled={running}
                title="Trigger Cycle"
                className={cn(
                  "p-1 rounded-lg text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer",
                  running && "animate-spin opacity-50"
                )}
              >
                <RefreshCw size={13} />
              </button>
              <button
                onClick={onClose}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors"
                aria-label="Close"
              >
                <X size={13} />
              </button>
            </div>
          </div>

          {/* Minimal Stages List */}
          <div className="flex flex-col gap-2">
            {STAGES.map((stage) => {
              const count = summary ? (summary as any)[stage.key] ?? 0 : 0;
              return (
                <div key={stage.key} className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span
                      className={cn(
                        "w-1.5 h-1.5 rounded-full shrink-0",
                        count > 0 ? "bg-[rgb(var(--accent))] animate-pulse" : "bg-white/10"
                      )}
                    />
                    <span className="text-[10px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]/70">
                      {stage.label}
                    </span>
                  </div>
                  <span
                    className={cn(
                      "text-[11px] font-mono font-bold",
                      count > 0 ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/30"
                    )}
                  >
                    {count}
                  </span>
                </div>
              );
            })}
          </div>

          {/* Trigger Action Pill */}
          <button
            onClick={handleTrigger}
            disabled={running}
            className="w-full py-1.5 rounded-xl border border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.08)] hover:bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] transition-all flex items-center justify-center gap-1.5 text-[10px] font-mono font-bold tracking-widest uppercase cursor-pointer disabled:opacity-40"
          >
            <Play size={10} fill="currentColor" />
            {running ? "Consolidating..." : "TRIGGER CYCLE"}
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
