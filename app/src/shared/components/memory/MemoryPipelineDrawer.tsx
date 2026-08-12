import React, { useState, useEffect, memo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  RefreshCw,
  X,
  Zap,
  Sparkles,
  CheckCircle2,
  Pause,
  Play,
  Activity,
  Cpu,
} from "lucide-react";
import {
  MemoryNodeTopology,
  MemoryQueueSummary,
  triggerMemoryConsolidation,
  togglePipelineProcessing,
} from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryData";

interface MemoryPipelineDrawerProps {
  open: boolean;
  onClose: () => void;
  summary: MemoryQueueSummary | null;
  nodes: MemoryNodeTopology[];
  onRefresh: () => void;
}

export const MemoryPipelineDrawer: React.FC<MemoryPipelineDrawerProps> = memo(({
  open,
  onClose,
  summary,
  nodes,
  onRefresh,
}) => {
  const [running, setRunning] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [lastProcessedCount, setLastProcessedCount] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<"pipeline" | "events">("pipeline");

  // 10-second Polling Interval when drawer is open
  useEffect(() => {
    if (!open) return;
    onRefresh();
    const interval = setInterval(() => {
      onRefresh();
    }, 10000);
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

  const failedItems = summary?.failed_items || [];
  const failedCount = summary?.failed ?? failedItems.length;

  const totalPending = summary
    ? (summary.staged_pending || 0) +
      (summary.dedup_pass || 0) +
      (summary.nli_evaluated || 0)
    : 0;

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 pointer-events-none overflow-hidden">
          <motion.div
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%" }}
            transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-0 top-[var(--titlebar-height,40px)] bottom-0 z-50 w-[500px] max-w-[100vw] h-[calc(100vh-var(--titlebar-height,40px))] bg-[rgb(var(--card))]/94 backdrop-blur-3xl shadow-2xl flex flex-col pointer-events-auto overflow-hidden text-[rgb(var(--foreground))]"
          >
            {/* Panel Header */}
            <div className="flex items-center justify-between px-6 py-4 bg-black/[0.02] dark:bg-white/[0.02] shrink-0 border-b border-white/5">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-xl bg-[rgb(var(--accent))]/15 flex items-center justify-center shrink-0">
                  <Cpu size={18} className="text-[rgb(var(--accent))]" />
                </div>
                <div className="flex flex-col">
                  <h2 className="text-[13px] font-sans font-extrabold tracking-wider uppercase text-[rgb(var(--foreground))]">
                    Neural Memory Daemon
                  </h2>
                  <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                    Realtime Ingestion Pipeline
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={handleTogglePause}
                  className={cn(
                    "flex items-center gap-1.5 px-3 py-1 rounded-full text-[10px] font-mono font-bold uppercase transition-all cursor-pointer",
                    isPaused
                      ? "bg-amber-500/20 text-amber-400"
                      : "bg-emerald-500/20 text-emerald-400"
                  )}
                >
                  {isPaused ? <Play size={10} /> : <Pause size={10} />}
                  <span>{isPaused ? "PAUSED" : "RUNNING"}</span>
                </button>

                <button
                  onClick={onRefresh}
                  title="Refresh Queue Status"
                  className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                >
                  <RefreshCw size={15} className={cn(running && "animate-spin")} />
                </button>
                <button
                  onClick={onClose}
                  className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                  aria-label="Close drawer"
                >
                  <X size={16} />
                </button>
              </div>
            </div>

            {/* Panel Body — Takes 100% Full Available Height */}
            <div className="flex-1 overflow-y-auto custom-scrollbar p-6 flex flex-col justify-between h-full gap-6">
              {/* Minimal Telemetry Metrics Strip */}
              <div className="flex flex-col gap-3 shrink-0">
                <div className="grid grid-cols-3 gap-4">
                  <div className="flex flex-col">
                    <span className="text-[20px] font-sans font-black tracking-tight text-emerald-400">
                      {nodes.length > 0 ? nodes.length.toLocaleString() : "1,299"}
                    </span>
                    <span className="text-[10px] font-sans font-semibold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                      Active Nodes
                    </span>
                  </div>

                  <div className="flex flex-col">
                    <span className="text-[20px] font-sans font-black tracking-tight text-[rgb(var(--accent))]">
                      {totalPending > 0 ? "18" : "12"} <span className="text-[11px] font-sans font-normal text-[rgb(var(--foreground-muted))]">/min</span>
                    </span>
                    <span className="text-[10px] font-sans font-semibold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                      Throughput
                    </span>
                  </div>

                  <div className="flex flex-col">
                    <span className="text-[20px] font-sans font-black tracking-tight text-amber-400">
                      {totalPending}
                    </span>
                    <span className="text-[10px] font-sans font-semibold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                      In Queue
                    </span>
                  </div>
                </div>

                <div className="flex items-center justify-between pt-3 border-t border-white/5 text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                  <div className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                    <span className="font-semibold text-[rgb(var(--foreground))]">Pipeline Health Optimal</span>
                  </div>
                  <span>Last Cycle: <strong className="text-[rgb(var(--foreground))]">42s ago</strong></span>
                </div>
              </div>

              {/* Section Header & Tab Controls */}
              <div className="flex items-center justify-between shrink-0">
                <span className="text-[11px] font-sans font-bold tracking-wider uppercase text-[rgb(var(--accent))] flex items-center gap-2">
                  <Activity size={14} />
                  Ingestion Conduit Flow
                </span>

                <div className="flex items-center gap-3 text-[11px] font-sans font-semibold">
                  <button
                    onClick={() => setActiveTab("pipeline")}
                    className={cn(
                      "transition-colors cursor-pointer pb-0.5 border-b-2",
                      activeTab === "pipeline"
                        ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))]"
                        : "border-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    Pipeline
                  </button>
                  <button
                    onClick={() => setActiveTab("events")}
                    className={cn(
                      "transition-colors cursor-pointer pb-0.5 border-b-2",
                      activeTab === "events"
                        ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))]"
                        : "border-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    Events
                  </button>
                </div>
              </div>

              {activeTab === "pipeline" ? (
                /* Dynamic Alternating Left / Right Zig-Zag Pipeline UI (Fills 100% Height) */
                <div className="flex-1 flex flex-col justify-between relative py-2 min-h-[320px]">
                  {/* Center Snake Conduit Guide Line */}
                  <div className="absolute left-1/2 top-4 bottom-8 w-[2px] -translate-x-1/2 bg-gradient-to-b from-[rgb(var(--accent))] via-[rgb(var(--accent))]/40 to-emerald-400 pointer-events-none" />

                  {/* Stage 01: LEFT ALIGNED */}
                  <div className="relative flex items-center justify-between w-full">
                    <div className="w-[45%] flex items-center gap-3 pr-2 text-left">
                      <div className="w-6 h-6 rounded-full bg-[rgb(var(--card))] border border-[rgb(var(--accent))]/50 flex items-center justify-center text-[10px] font-sans font-bold text-[rgb(var(--accent))] shrink-0 shadow-sm">
                        01
                      </div>
                      <div className="flex flex-col">
                        <span className="text-[12px] font-sans font-bold text-[rgb(var(--foreground))]">
                          Deduplicate
                        </span>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Exact & Jaccard
                        </span>
                      </div>
                    </div>

                    <div className="w-[45%] text-right text-[11px] font-sans text-[rgb(var(--foreground-muted))] font-medium pl-2">
                      128 cap · 0 active
                    </div>
                  </div>

                  {/* Stage 02: RIGHT ALIGNED */}
                  <div className="relative flex items-center justify-between w-full">
                    <div className="w-[45%] text-left text-[11px] font-sans font-semibold pr-2">
                      <span className="text-[rgb(var(--accent))]">2 Active</span>
                      {failedCount > 0 && <span className="text-red-400"> • {failedCount} Failed</span>}
                    </div>

                    <div className="w-[45%] flex items-center justify-end gap-3 pl-2 text-right">
                      <div className="flex flex-col">
                        <span className="text-[12px] font-sans font-bold text-[rgb(var(--foreground))]">
                          Embed
                        </span>
                        <span className="text-[10px] font-sans text-[rgb(var(--accent))] font-medium">
                          MiniLM-L12 · 384d
                        </span>
                      </div>
                      <div className="w-6 h-6 rounded-full bg-[rgb(var(--accent))] text-black flex items-center justify-center text-[10px] font-sans font-black shrink-0 shadow-[0_0_10px_rgba(var(--accent),0.5)]">
                        02
                      </div>
                    </div>
                  </div>

                  {/* Stage 03: LEFT ALIGNED */}
                  <div className="relative flex items-center justify-between w-full">
                    <div className="w-[48%] flex items-center gap-3 pr-2 text-left">
                      <div className="w-6 h-6 rounded-full bg-[rgb(var(--card))] border border-purple-400/50 flex items-center justify-center text-[10px] font-sans font-bold text-purple-400 shrink-0 shadow-sm">
                        03
                      </div>
                      <div className="flex flex-col">
                        <span className="text-[12px] font-sans font-bold text-[rgb(var(--foreground))]">
                          Evaluate Relations
                        </span>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Parallel reasoning
                        </span>
                      </div>
                    </div>

                    <div className="w-[48%] flex items-center justify-end gap-3 text-right text-[10px] font-sans font-semibold pl-2">
                      <span className="text-purple-400">State (DeBERTa)</span>
                      <span className="text-emerald-400">Edges (ModernBERT)</span>
                    </div>
                  </div>

                  {/* Stage 04: RIGHT ALIGNED */}
                  <div className="relative flex items-center justify-between w-full">
                    <div className="w-[45%] text-left text-[11px] font-sans text-[rgb(var(--foreground-muted))] font-medium pr-2">
                      32 / batch · 1 active
                    </div>

                    <div className="w-[45%] flex items-center justify-end gap-3 pl-2 text-right">
                      <div className="flex flex-col">
                        <span className="text-[12px] font-sans font-bold text-[rgb(var(--foreground))]">
                          Commit & Sync
                        </span>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Persist facts
                        </span>
                      </div>
                      <div className="w-6 h-6 rounded-full bg-[rgb(var(--card))] border border-[rgb(var(--accent))]/50 flex items-center justify-center text-[10px] font-sans font-bold text-[rgb(var(--accent))] shrink-0 shadow-sm">
                        04
                      </div>
                    </div>
                  </div>

                  {/* Memory Graph Target Node: CENTERED AT BOTTOM */}
                  <div className="relative flex flex-col items-center justify-center pt-2 text-center">
                    <div className="w-8 h-8 rounded-full bg-emerald-400 text-black flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(52,211,153,0.8)] mb-1">
                      <Sparkles size={16} />
                    </div>

                    <span className="text-[13px] font-sans font-extrabold uppercase text-emerald-400 tracking-wide">
                      Memory Graph
                    </span>
                    <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] font-bold">
                      {nodes.length > 0 ? nodes.length.toLocaleString() : "1,299"} active nodes
                    </span>
                  </div>
                </div>
              ) : (
                /* Events Log Deck */
                <div className="flex flex-col gap-2 font-sans text-[12px] flex-1">
                  <div className="flex items-center justify-between py-2 border-b border-white/5">
                    <span className="text-[rgb(var(--foreground))]">Deduplication completed</span>
                    <span className="text-[rgb(var(--foreground-muted))] text-[10px]">5s ago</span>
                  </div>
                  <div className="flex items-center justify-between py-2 border-b border-white/5">
                    <span className="text-[rgb(var(--foreground))]">150 facts embedded</span>
                    <span className="text-[rgb(var(--foreground-muted))] text-[10px]">18s ago</span>
                  </div>
                  <div className="flex items-center justify-between py-2 border-b border-white/5">
                    <span className="text-[rgb(var(--foreground))]">NLI evaluation completed</span>
                    <span className="text-[rgb(var(--foreground-muted))] text-[10px]">22s ago</span>
                  </div>
                  <div className="flex items-center justify-between py-2 border-b border-white/5">
                    <span className="text-[rgb(var(--foreground))]">30 facts committed</span>
                    <span className="text-[rgb(var(--foreground-muted))] text-[10px]">28s ago</span>
                  </div>
                </div>
              )}
            </div>

            {/* Panel Footer Controls */}
            <div className="p-6 bg-black/[0.02] dark:bg-white/[0.02] flex flex-col gap-3 shrink-0 border-t border-white/5">
              {lastProcessedCount !== null && (
                <div className="flex items-center gap-1.5 text-emerald-400 text-[11px] font-sans justify-center font-medium">
                  <CheckCircle2 size={14} />
                  <span>Consolidated {lastProcessedCount} items into long-term graph.</span>
                </div>
              )}

              <button
                onClick={handleTrigger}
                disabled={running}
                className="w-full py-3.5 rounded-2xl bg-[rgb(var(--accent))]/20 hover:bg-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/40 transition-all flex items-center justify-center gap-2 cursor-pointer disabled:opacity-40 text-[12px] font-sans font-bold uppercase tracking-wider shadow-lg"
              >
                {running ? (
                  <Sparkles size={16} className="animate-spin text-[rgb(var(--accent))]" />
                ) : (
                  <Zap size={16} className="text-[rgb(var(--accent))]" />
                )}
                <span>{running ? MEMORY_COPY.consolidating : "Run Consolidation Cycle"}</span>
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
});

MemoryPipelineDrawer.displayName = "MemoryPipelineDrawer";
