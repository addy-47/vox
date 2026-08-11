import React, { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Cpu,
  X,
  RefreshCw,
  Layers,
  AlertCircle,
  CheckCircle2,
  Sparkles,
  Zap,
  RotateCcw,
  Pause,
  Play,
} from "lucide-react";
import {
  MemoryNodeTopology,
  MemoryEdgeTopology,
  MemoryQueueSummary,
  triggerMemoryConsolidation,
  retryFailedQueue,
  togglePipelineProcessing,
} from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

interface MemoryPipelineDrawerProps {
  open: boolean;
  onClose: () => void;
  summary: MemoryQueueSummary | null;
  nodes: MemoryNodeTopology[];
  edges: MemoryEdgeTopology[];
  onRefresh: () => void;
}

const STAGES = [
  {
    key: "staged_pending",
    title: "1. Staged Deduplication",
    badge: "Stage 1",
    desc: "Filters out verbatim and sub-word exact duplicate facts",
  },
  {
    key: "dedup_pass",
    title: "2. Vector Embedding",
    badge: "Stage 2",
    desc: "Generates semantic vector embeddings for fast retrieval",
  },
  {
    key: "nli_evaluated",
    title: "3. NLI Fact Reasoning",
    badge: "Stage 3",
    desc: "Evaluates DeBERTa-v3 state replacement and directed graph edges",
  },
  {
    key: "paused",
    title: "4. Knowledge Storage",
    badge: "Stage 4",
    desc: "Persists verified facts and relation edges into long-term graph",
  },
] as const;

function formatElapsed(created_at: number): string {
  const diffSec = Math.max(0, Math.floor((Date.now() - created_at) / 1000));
  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  return `${Math.floor(diffMin / 60)}h ago`;
}

export const MemoryPipelineDrawer: React.FC<MemoryPipelineDrawerProps> = ({
  open,
  onClose,
  summary,
  nodes,
  edges,
  onRefresh,
}) => {
  const [running, setRunning] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [lastProcessedCount, setLastProcessedCount] = useState<number | null>(null);

  const totalPending = summary
    ? (summary.staged_pending ?? 0) +
      (summary.dedup_pass ?? 0) +
      (summary.nli_evaluated ?? 0) +
      (summary.paused ?? 0)
    : 0;

  const handleTrigger = async () => {
    setRunning(true);
    setLastProcessedCount(null);
    try {
      const count = await triggerMemoryConsolidation();
      setLastProcessedCount(count);
      setTimeout(onRefresh, 500);
    } catch (e) {
      console.error("Consolidation trigger failed:", e);
    } finally {
      setRunning(false);
    }
  };

  const handleRetryFailed = async () => {
    setRetrying(true);
    try {
      await retryFailedQueue();
      onRefresh();
    } catch (e) {
      console.error("Retry failed items failed:", e);
    } finally {
      setRetrying(false);
    }
  };

  const handleTogglePause = async () => {
    try {
      const paused = await togglePipelineProcessing();
      setIsPaused(paused);
      onRefresh();
    } catch (e) {
      console.error("Toggle pipeline processing failed:", e);
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <>
          {/* Backdrop overlay */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="fixed inset-0 z-40 bg-black/40 backdrop-blur-[2px]"
          />

          {/* Bottom-Right Slide-out Panel */}
          <motion.div
            initial={{ opacity: 0, scale: 0.96, y: 16 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: 16 }}
            transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-4 bottom-4 z-50 w-[420px] max-w-[94vw] max-h-[88vh] glass-card border border-[rgba(var(--accent),0.2)] bg-[rgba(10,12,14,0.95)] backdrop-blur-2xl shadow-[0_20px_60px_rgba(0,0,0,0.6)] rounded-3xl flex flex-col pointer-events-auto overflow-hidden"
          >
            {/* Panel Header */}
            <div className="flex items-center justify-between px-5 py-4 border-b border-white/[0.08] bg-white/[0.01]">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center shrink-0">
                  <Cpu size={16} className="text-[rgb(var(--accent))]" />
                </div>
                <div>
                  <h2 className="text-[13px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground))]">
                    Memory Ingestion Queue
                  </h2>
                  <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/70">
                    Live Background Ingestion Status
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-1.5">
                <button
                  onClick={onRefresh}
                  title="Refresh Ingestion Queue Status"
                  className="p-1.5 rounded-xl text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer"
                >
                  <RefreshCw size={14} className={cn(running && "animate-spin")} />
                </button>
                <button
                  onClick={onClose}
                  className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-white/[0.06] transition-colors cursor-pointer"
                >
                  <X size={16} />
                </button>
              </div>
            </div>

            {/* Panel Content Body */}
            <div className="flex-1 overflow-y-auto custom-scrollbar p-5 space-y-4">
              {/* Live Status Badge Card */}
              <div className="p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.07] flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div
                    className={cn(
                      "w-3 h-3 rounded-full shrink-0",
                      isPaused
                        ? "bg-amber-400"
                        : totalPending > 0
                        ? "bg-[rgb(var(--accent))] animate-pulse"
                        : "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.5)]"
                    )}
                  />
                  <div>
                    <span className="text-[12px] font-sans font-semibold text-[rgb(var(--foreground))] block">
                      {isPaused
                        ? "Pipeline Processing Paused"
                        : totalPending > 0
                        ? `${totalPending} ${totalPending === 1 ? "Fact" : "Facts"} Queued for Ingestion`
                        : "Memory Ingestion Synchronized"}
                    </span>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60">
                      {isPaused
                        ? "Background worker paused by user"
                        : totalPending > 0
                        ? "Auto-consolidating background queue..."
                        : "Ready · Idle background daemon"}
                    </span>
                  </div>
                </div>

                <span
                  className={cn(
                    "px-2.5 py-1 rounded-full text-[9px] font-mono font-bold tracking-wider uppercase shrink-0",
                    isPaused
                      ? "bg-amber-500/15 text-amber-400 border border-amber-500/30"
                      : totalPending > 0
                      ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30"
                      : "bg-emerald-500/15 text-emerald-400 border border-emerald-500/30"
                  )}
                >
                  {isPaused ? "PAUSED" : totalPending > 0 ? "PROCESSING" : "READY"}
                </span>
              </div>

              {/* Vertical 4-Stage Timeline */}
              <div className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/80">
                    Ingestion Timeline
                  </span>
                  <span className="text-[10px] font-mono text-[rgb(var(--accent))] font-medium">
                    4-Stage Pipeline
                  </span>
                </div>

                <div className="flex flex-col gap-2">
                  {STAGES.map((stage, idx) => {
                    const count = summary ? (summary as any)[stage.key] ?? 0 : 0;
                    return (
                      <div
                        key={stage.key}
                        className="p-3 rounded-2xl bg-white/[0.02] border border-white/[0.05] hover:border-[rgba(var(--accent),0.2)] transition-all flex items-start justify-between gap-3"
                      >
                        <div className="flex items-start gap-2.5">
                          <div className="w-6 h-6 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 flex items-center justify-center font-mono text-[10px] font-bold text-[rgb(var(--accent))] shrink-0 mt-0.5">
                            {idx + 1}
                          </div>
                          <div className="flex flex-col">
                            <span className="text-[11px] font-sans font-bold text-[rgb(var(--foreground))]">
                              {stage.title}
                            </span>
                            <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]/60 leading-normal">
                              {stage.desc}
                            </span>
                          </div>
                        </div>

                        <span
                          className={cn(
                            "px-2 py-0.5 rounded-lg text-[11px] font-mono font-bold shrink-0",
                            count > 0
                              ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))]"
                              : "bg-white/[0.03] text-[rgb(var(--foreground-muted))]/40"
                          )}
                        >
                          {count}
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Knowledge Base Totals */}
              <div className="p-3 rounded-2xl bg-white/[0.02] border border-white/[0.05] flex items-center justify-between text-[11px] font-mono">
                <span className="text-[rgb(var(--foreground-muted))]/80">Graph Topology Totals:</span>
                <div className="flex items-center gap-2 font-bold">
                  <span className="text-[rgb(var(--accent))]">{nodes.length} Nodes</span>
                  <span className="text-white/20">|</span>
                  <span className="text-emerald-400">{edges.length} Edges</span>
                </div>
              </div>

              {/* Live Queue Activity Stream */}
              <div className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <div className="flex items-center gap-1.5">
                    <Layers size={13} className="text-[rgb(var(--accent))]" />
                    <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/80">
                      Recent Queue Items
                    </span>
                  </div>
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50">
                    Live Stream
                  </span>
                </div>

                {summary?.recent_items && summary.recent_items.length > 0 ? (
                  <div className="space-y-2 max-h-[160px] overflow-y-auto custom-scrollbar pr-1">
                    {summary.recent_items.slice(0, 10).map((item) => (
                      <div
                        key={item.id}
                        className="p-2.5 rounded-2xl bg-white/[0.02] border border-white/[0.05] flex flex-col gap-1 text-[11px]"
                      >
                        <div className="flex items-center justify-between">
                          <span className="font-mono font-semibold text-[9px] uppercase tracking-wider text-[rgb(var(--accent))]">
                            {item.status.replace("_", " ")}
                          </span>
                          <span className="font-mono text-[10px] text-[rgb(var(--foreground-muted))]/50">
                            {formatElapsed(item.created_at)}
                          </span>
                        </div>
                        <p className="text-[11px] font-normal text-[rgb(var(--foreground))]/90 truncate">
                          "{item.fact}"
                        </p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="p-4 rounded-2xl bg-white/[0.02] border border-white/[0.04] text-center">
                    <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/50 italic">
                      Ingestion queue clean · All items processed
                    </span>
                  </div>
                )}
              </div>

              {/* Failed Items Banner & Retry Button */}
              {summary?.failed !== undefined && summary.failed > 0 && (
                <div className="p-3 rounded-2xl bg-red-500/10 border border-red-500/20 text-red-400 flex items-center justify-between text-[11px] font-mono">
                  <div className="flex items-center gap-2">
                    <AlertCircle size={14} />
                    <span className="font-bold uppercase tracking-wider">
                      {summary.failed} Failed Queue Items
                    </span>
                  </div>
                  <button
                    onClick={handleRetryFailed}
                    disabled={retrying}
                    className="flex items-center gap-1 px-2.5 py-1 rounded-xl bg-red-500/20 hover:bg-red-500/30 text-red-200 border border-red-500/40 text-[10px] font-bold uppercase transition-colors cursor-pointer"
                  >
                    <RotateCcw size={12} className={cn(retrying && "animate-spin")} />
                    <span>{retrying ? "Retrying..." : "Retry Failed"}</span>
                  </button>
                </div>
              )}
            </div>

            {/* Footer Controls */}
            <div className="p-4 border-t border-white/[0.08] bg-white/[0.01] flex flex-col gap-2">
              {lastProcessedCount !== null && (
                <div className="flex items-center gap-1.5 text-emerald-400 text-[11px] font-mono justify-center">
                  <CheckCircle2 size={13} />
                  <span>Consolidated {lastProcessedCount} items into long-term graph.</span>
                </div>
              )}

              <div className="grid grid-cols-2 gap-2">
                <button
                  onClick={handleTogglePause}
                  className="py-2.5 rounded-2xl border border-white/[0.12] bg-white/[0.04] hover:bg-white/[0.08] text-[rgb(var(--foreground))] transition-all flex items-center justify-center gap-2 text-[10px] font-mono font-bold tracking-wider uppercase cursor-pointer"
                >
                  {isPaused ? <Play size={13} /> : <Pause size={13} />}
                  <span>{isPaused ? "Resume Pipeline" : "Pause Pipeline"}</span>
                </button>

                <button
                  onClick={handleTrigger}
                  disabled={running}
                  className="py-2.5 rounded-2xl border border-[rgba(var(--accent),0.35)] bg-[rgba(var(--accent),0.12)] hover:bg-[rgba(var(--accent),0.22)] text-[rgb(var(--accent))] transition-all flex items-center justify-center gap-2 text-[10px] font-mono font-bold tracking-wider uppercase cursor-pointer disabled:opacity-40 shadow-lg"
                >
                  {running ? (
                    <Sparkles size={13} className="animate-spin text-[rgb(var(--accent))]" />
                  ) : (
                    <Zap size={13} className="text-[rgb(var(--accent))]" />
                  )}
                  <span>{running ? "Processing..." : "Run Consolidation"}</span>
                </button>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
};
