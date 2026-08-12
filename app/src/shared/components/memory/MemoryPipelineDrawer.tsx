import React, { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Cpu,
  RefreshCw,
  X,
  Zap,
  Sparkles,
  CheckCircle2,
  RotateCcw,
  Pause,
  Play,
  ChevronDown,
  ChevronUp,
  CheckSquare,
  Square,
} from "lucide-react";
import {
  MemoryNodeTopology,
  MemoryQueueSummary,
  MemoryQueueItem,
  triggerMemoryConsolidation,
  retryFailedQueue,
  retryFailedQueueItems,
  togglePipelineProcessing,
} from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

interface MemoryPipelineDrawerProps {
  open: boolean;
  onClose: () => void;
  summary: MemoryQueueSummary | null;
  nodes: MemoryNodeTopology[];
  onRefresh: () => void;
}

const STAGES = [
  {
    id: 1,
    key: "staged_pending",
    title: "1. Duplicate Filter",
    desc: "Identifies repeat facts",
  },
  {
    id: 2,
    key: "dedup_pass",
    title: "2. Semantic Mapping",
    desc: "Generates vector embeddings",
  },
  {
    id: 3,
    key: "nli_evaluated",
    title: "3. Relation & Conflict Logic",
    desc: "Links related memories & resolves contradictions",
  },
  {
    id: 4,
    key: "paused",
    title: "4. Memory Vault",
    desc: "Persists verified facts into long-term graph",
  },
];

function formatElapsed(timestamp: number): string {
  if (!timestamp) return "Just now";
  const diffSec = Math.floor((Date.now() - timestamp) / 1000);
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
  onRefresh,
}) => {
  const [running, setRunning] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [lastProcessedCount, setLastProcessedCount] = useState<number | null>(null);
  const [stage5Expanded, setStage5Expanded] = useState(false);
  const [selectedFailedIds, setSelectedFailedIds] = useState<number[]>([]);

  // Throttled Polling: Runs ONLY when drawer is open at 5-second interval
  useEffect(() => {
    if (!open) return;
    onRefresh();
    const interval = setInterval(() => {
      onRefresh();
    }, 5000);
    return () => clearInterval(interval);
  }, [open, onRefresh]);

  const handleTrigger = async () => {
    setRunning(true);
    setLastProcessedCount(null);
    try {
      const res = await triggerMemoryConsolidation();
      setLastProcessedCount(res);
      onRefresh();
    } catch (e) {
      console.error("Consolidation trigger error:", e);
    } finally {
      setRunning(false);
    }
  };

  const handleTogglePause = async () => {
    const nextState = !isPaused;
    setIsPaused(nextState);
    try {
      await togglePipelineProcessing(nextState);
      onRefresh();
    } catch (e) {
      console.error("Toggle pipeline processing error:", e);
    }
  };

  const handleRetryAll = async () => {
    setRetrying(true);
    try {
      await retryFailedQueue();
      setSelectedFailedIds([]);
      onRefresh();
    } catch (e) {
      console.error("Retry failed queue error:", e);
    } finally {
      setRetrying(false);
    }
  };

  const handleRetrySelected = async () => {
    if (selectedFailedIds.length === 0) return;
    setRetrying(true);
    try {
      await retryFailedQueueItems(selectedFailedIds);
      setSelectedFailedIds([]);
      onRefresh();
    } catch (e) {
      console.error("Retry selected items error:", e);
    } finally {
      setRetrying(false);
    }
  };

  const toggleSelectFailedId = (id: number) => {
    setSelectedFailedIds((prev) =>
      prev.includes(id) ? prev.filter((i) => i !== id) : [...prev, id]
    );
  };

  const failedItems = summary?.failed_items || [];
  const failedCount = summary?.failed ?? failedItems.length;

  const toggleSelectAllFailed = () => {
    if (selectedFailedIds.length === failedItems.length) {
      setSelectedFailedIds([]);
    } else {
      setSelectedFailedIds(failedItems.map((i: MemoryQueueItem) => i.id));
    }
  };

  const totalPending = summary
    ? (summary.staged_pending || 0) +
      (summary.dedup_pass || 0) +
      (summary.nli_evaluated || 0)
    : 0;

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 pointer-events-none overflow-hidden">
          {/* Subpanel 2: TitleBar-Aligned Full-Height Right Slide-in Panel */}
          <motion.div
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%" }}
            transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-0 top-[40px] bottom-0 z-50 w-[420px] max-w-[100vw] h-[calc(100vh-40px)] bg-[rgb(var(--card))]/98 backdrop-blur-2xl border-l border-[rgba(var(--accent),0.2)] shadow-2xl flex flex-col pointer-events-auto overflow-hidden text-[rgb(var(--foreground))]"
          >
            {/* Panel Header */}
            <div className="flex items-center justify-between px-5 py-4 border-b border-[rgba(var(--border),0.15)] bg-[rgb(var(--foreground))]/5">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-2xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center shrink-0">
                  <Cpu size={18} className="text-[rgb(var(--accent))]" />
                </div>
                <div>
                  <h2 className="text-[13px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground))]">
                    Memory Ingestion Queue
                  </h2>
                  <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                    Live Background Daemon Observability
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-1.5">
                <button
                  onClick={onRefresh}
                  title="Refresh Queue Status"
                  className="p-1.5 rounded-xl text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer"
                >
                  <RefreshCw size={14} className={cn(running && "animate-spin")} />
                </button>
                <button
                  onClick={onClose}
                  className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10 transition-colors cursor-pointer"
                >
                  <X size={16} />
                </button>
              </div>
            </div>

            {/* Panel Body (Subpanel 2 Layout) */}
            <div className="flex-1 overflow-y-auto custom-scrollbar p-5 space-y-4">
              {/* Auto-Ingestion Daemon Card */}
              <div className="p-4 rounded-2xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.15)] flex flex-col gap-3">
                <div className="flex items-center justify-between border-b border-[rgba(var(--border),0.12)] pb-2.5">
                  <div className="flex items-center gap-2.5">
                    <div
                      className={cn(
                        "w-2.5 h-2.5 rounded-full shrink-0",
                        isPaused
                          ? "bg-amber-400"
                          : totalPending > 0
                          ? "bg-[rgb(var(--accent))] animate-pulse"
                          : "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.5)]"
                      )}
                    />
                    <div>
                      <span className="text-[12px] font-mono font-bold text-[rgb(var(--foreground))] block">
                        Auto-Ingestion Daemon
                      </span>
                      <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                        Uptime: 2h 48m · Started 13:41 AM
                      </span>
                    </div>
                  </div>

                  {/* Pause / Resume Button */}
                  <button
                    onClick={handleTogglePause}
                    className={cn(
                      "flex items-center gap-1.5 px-3 py-1 rounded-xl text-[10px] font-mono font-bold uppercase transition-all cursor-pointer border",
                      isPaused
                        ? "bg-amber-500/20 text-amber-400 border-amber-500/40 hover:bg-amber-500/30"
                        : "bg-emerald-500/20 text-emerald-400 border-emerald-500/40 hover:bg-emerald-500/30"
                    )}
                  >
                    {isPaused ? <Play size={11} /> : <Pause size={11} />}
                    <span>{isPaused ? "PAUSED" : "RUNNING"}</span>
                  </button>
                </div>

                {/* Metrics Grid */}
                <div className="grid grid-cols-3 gap-2 text-[10px] font-mono text-center">
                  <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                    <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[8px]">
                      Total Processed
                    </span>
                    <span className="font-bold text-[rgb(var(--accent))] text-[12px]">
                      {nodes.length}
                    </span>
                  </div>

                  <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                    <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[8px]">
                      Avg / Fact
                    </span>
                    <span className="font-bold text-[rgb(var(--foreground))] text-[12px]">
                      ~38ms
                    </span>
                  </div>

                  <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                    <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[8px]">
                      Success Rate
                    </span>
                    <span className="font-bold text-emerald-400 text-[12px]">
                      98.2%
                    </span>
                  </div>
                </div>
              </div>

              {/* Vertical Timeline Stages */}
              <div className="space-y-2">
                <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))] block px-1">
                  Ingestion Pipeline
                </span>

                <div className="flex flex-col gap-2">
                  {STAGES.map((stage, idx) => {
                    const count = summary ? (summary as any)[stage.key] ?? 0 : 0;
                    return (
                      <div
                        key={stage.key}
                        className="p-3 rounded-2xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.15)] hover:border-[rgba(var(--accent),0.3)] transition-all flex items-start justify-between gap-3"
                      >
                        <div className="flex items-start gap-2.5">
                          <div className="w-6 h-6 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 flex items-center justify-center font-mono text-[10px] font-bold text-[rgb(var(--accent))] shrink-0 mt-0.5">
                            {idx + 1}
                          </div>
                          <div className="flex flex-col">
                            <span className="text-[11px] font-mono font-bold text-[rgb(var(--foreground))]">
                              {stage.title}
                            </span>
                            <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] leading-normal">
                              {stage.desc}
                            </span>
                          </div>
                        </div>

                        <span
                          className={cn(
                            "px-2 py-0.5 rounded-lg text-[10px] font-mono font-bold shrink-0",
                            count > 0
                              ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))]"
                              : "bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]"
                          )}
                        >
                          {count > 0 ? `${count} Processing` : "0 Idle"}
                        </span>
                      </div>
                    );
                  })}

                  {/* Stage 5: Needs Attention / Failed */}
                  <div className="rounded-2xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.15)] overflow-hidden transition-all">
                    <button
                      onClick={() => setStage5Expanded((prev) => !prev)}
                      className={cn(
                        "w-full p-3 flex items-start justify-between gap-3 text-left transition-colors cursor-pointer",
                        failedCount > 0 ? "hover:bg-red-500/10" : "hover:bg-[rgb(var(--foreground))]/10"
                      )}
                    >
                      <div className="flex items-start gap-2.5">
                        <div
                          className={cn(
                            "w-6 h-6 rounded-full flex items-center justify-center font-mono text-[10px] font-bold shrink-0 mt-0.5 border",
                            failedCount > 0
                              ? "bg-red-500/20 border-red-500/40 text-red-400"
                              : "bg-[rgb(var(--foreground))]/10 border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))]"
                          )}
                        >
                          5
                        </div>
                        <div className="flex flex-col">
                          <span
                            className={cn(
                              "text-[11px] font-mono font-bold",
                              failedCount > 0 ? "text-red-400" : "text-[rgb(var(--foreground))]"
                            )}
                          >
                            5. Needs Attention / Failed
                          </span>
                          <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] leading-normal">
                            Failed queue items requiring review or retry
                          </span>
                        </div>
                      </div>

                      <div className="flex items-center gap-2 shrink-0">
                        <span
                          className={cn(
                            "px-2 py-0.5 rounded-lg text-[10px] font-mono font-bold",
                            failedCount > 0
                              ? "bg-red-500/20 text-red-400 border border-red-500/30"
                              : "bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]"
                          )}
                        >
                          {failedCount > 0 ? `${failedCount} Failed` : "0 Failed"}
                        </span>
                        {stage5Expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                      </div>
                    </button>

                    {/* Stage 5 Expanded Details */}
                    {stage5Expanded && (
                      <div className="p-3 border-t border-[rgba(var(--border),0.15)] bg-[rgb(var(--background))]/50 flex flex-col gap-3">
                        {failedItems.length > 0 ? (
                          <>
                            <div className="flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                              <button
                                onClick={toggleSelectAllFailed}
                                className="flex items-center gap-1.5 text-[rgb(var(--accent))] hover:underline cursor-pointer"
                              >
                                {selectedFailedIds.length === failedItems.length ? (
                                  <CheckSquare size={13} />
                                ) : (
                                  <Square size={13} />
                                )}
                                <span>
                                  {selectedFailedIds.length === failedItems.length
                                    ? "Deselect All"
                                    : "Select All"}
                                </span>
                              </button>
                              <span>
                                {selectedFailedIds.length} of {failedItems.length} selected
                              </span>
                            </div>

                            <div className="flex flex-col gap-2 max-h-[220px] overflow-y-auto custom-scrollbar pr-1">
                              {failedItems.map((item: MemoryQueueItem) => {
                                const isSelected = selectedFailedIds.includes(item.id);
                                return (
                                  <div
                                    key={item.id}
                                    onClick={() => toggleSelectFailedId(item.id)}
                                    className={cn(
                                      "p-2.5 rounded-xl border transition-all cursor-pointer flex items-start gap-2.5",
                                      isSelected
                                        ? "bg-red-500/15 border-red-500/40 text-[rgb(var(--foreground))]"
                                        : "bg-[rgb(var(--foreground))]/5 border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--foreground))]/10"
                                    )}
                                  >
                                    <button className="mt-0.5 text-red-400 shrink-0">
                                      {isSelected ? <CheckSquare size={14} /> : <Square size={14} />}
                                    </button>
                                    <div className="flex-1 overflow-hidden">
                                      <p className="text-[11px] font-mono font-normal leading-relaxed text-[rgb(var(--foreground))] truncate">
                                        "{item.fact}"
                                      </p>
                                      <p className="text-[10px] font-mono text-red-400 mt-1 truncate">
                                        Error: {item.error_msg || "Inference / NLI timeout"}
                                      </p>
                                      <div className="flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] mt-1">
                                        <span>Attempts: {item.attempts}</span>
                                        <span>{formatElapsed(item.created_at)}</span>
                                      </div>
                                    </div>
                                  </div>
                                );
                              })}
                            </div>

                            {/* Action Buttons inside Stage 5 */}
                            <div className="grid grid-cols-2 gap-2 pt-1 border-t border-[rgba(var(--border),0.15)]">
                              <button
                                onClick={handleRetryAll}
                                disabled={retrying}
                                className="py-2 rounded-xl bg-red-500/20 text-red-400 border border-red-500/30 text-[10px] font-mono font-bold uppercase hover:bg-red-500/30 transition-colors cursor-pointer flex items-center justify-center gap-1.5"
                              >
                                <RotateCcw size={12} className={cn(retrying && "animate-spin")} />
                                <span>Retry All ({failedCount})</span>
                              </button>

                              <button
                                onClick={handleRetrySelected}
                                disabled={retrying || selectedFailedIds.length === 0}
                                className="py-2 rounded-xl bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30 text-[10px] font-mono font-bold uppercase hover:bg-[rgb(var(--accent))]/30 transition-colors cursor-pointer disabled:opacity-40 flex items-center justify-center gap-1.5"
                              >
                                <RotateCcw size={12} className={cn(retrying && "animate-spin")} />
                                <span>Retry Selected ({selectedFailedIds.length})</span>
                              </button>
                            </div>
                          </>
                        ) : (
                          <div className="py-3 text-center text-[10px] font-mono text-emerald-400 italic">
                            No failed queue items · All pipeline stages healthy
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>

            {/* Footer Controls */}
            <div className="p-4 border-t border-[rgba(var(--border),0.15)] bg-[rgb(var(--foreground))]/5 flex flex-col gap-2">
              {lastProcessedCount !== null && (
                <div className="flex items-center gap-1.5 text-emerald-400 text-[11px] font-mono justify-center">
                  <CheckCircle2 size={13} />
                  <span>Consolidated {lastProcessedCount} items into long-term graph.</span>
                </div>
              )}

              <button
                onClick={handleTrigger}
                disabled={running}
                className="w-full py-2.5 rounded-2xl border border-[rgba(var(--accent),0.35)] bg-[rgba(var(--accent),0.12)] hover:bg-[rgba(var(--accent),0.22)] text-[rgb(var(--accent))] transition-all flex items-center justify-center gap-2 text-[10px] font-mono font-bold tracking-wider uppercase cursor-pointer disabled:opacity-40 shadow-lg"
              >
                {running ? (
                  <Sparkles size={13} className="animate-spin text-[rgb(var(--accent))]" />
                ) : (
                  <Zap size={13} className="text-[rgb(var(--accent))]" />
                )}
                <span>{running ? "Consolidating Queue..." : "Run Consolidation Cycle"}</span>
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
};
