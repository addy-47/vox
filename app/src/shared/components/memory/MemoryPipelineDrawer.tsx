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
  Filter,
  Box,
  GitBranch,
  Database,
  ShieldAlert,
  TrendingUp,
  Layers,
  Clock,
  ShieldCheck,
  Brain,
  AlertCircle,
  RotateCcw,
  Activity,
  Check,
} from "lucide-react";
import {
  MemoryNodeTopology,
  MemoryEdgeTopology,
  MemoryQueueSummary,
  triggerMemoryConsolidation,
  togglePipelineProcessing,
  retryFailedQueue,
  retryFailedQueueItems,
} from "@/services/memoryService";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryData";

interface MemoryPipelineDrawerProps {
  open: boolean;
  onClose: () => void;
  summary: MemoryQueueSummary | null;
  nodes: MemoryNodeTopology[];
  edges?: MemoryEdgeTopology[];
  onRefresh: () => void;
}

export const MemoryPipelineDrawer: React.FC<MemoryPipelineDrawerProps> = memo(({
  open,
  onClose,
  summary,
  nodes,
  edges = [],
  onRefresh,
}) => {
  const [running, setRunning] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [lastProcessedCount, setLastProcessedCount] = useState<number | null>(null);
  const [lastRetriedCount, setLastRetriedCount] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<"pipeline" | "failed">("pipeline");

  // Settings Store SSOT for Pipeline Processing Enabled
  const pipelineProcessingEnabled = useSettingsStore((s) => s.settings?.memory?.pipeline_processing_enabled ?? true);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

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
      console.error("Pipeline sweep error:", e);
    } finally {
      setRunning(false);
    }
  };

  const handleTogglePause = async () => {
    try {
      const nextState = await togglePipelineProcessing();
      updateDraft("memory", "pipeline_processing_enabled", nextState);
      await commitChanges();
      onRefresh();
    } catch (e) {
      console.error("Toggle pipeline processing error:", e);
    }
  };

  const handleRetryAll = async () => {
    setRetrying(true);
    setLastRetriedCount(null);
    try {
      const count = await retryFailedQueue();
      setLastRetriedCount(count);
      onRefresh();
    } catch (e) {
      console.error("Retry failed queue error:", e);
    } finally {
      setRetrying(false);
    }
  };

  const handleRetrySingleItem = async (itemId: number) => {
    try {
      await retryFailedQueueItems([itemId]);
      onRefresh();
    } catch (e) {
      console.error("Retry item error:", e);
    }
  };

  // Real backend metrics from SQLite personal_memory_queue
  const stagedPendingCount = summary?.staged_pending ?? 0;
  const dedupPassCount = summary?.dedup_pass ?? 0;
  const nliEvaluatedCount = summary?.nli_evaluated ?? 0;
  const failedCount = summary?.failed ?? (summary?.failed_items?.length || 0);

  const totalPending = stagedPendingCount + dedupPassCount + nliEvaluatedCount;
  const activeNodesCount = nodes.length;
  const activeEdgesCount = edges.length;

  const recentItems = summary?.recent_items || [];
  const failedQueueItems = recentItems.filter((item) => item.status === "failed" || item.error_msg);

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 pointer-events-none overflow-hidden select-none">
          <motion.div
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%" }}
            transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-0 top-[var(--titlebar-height,40px)] bottom-0 z-50 w-[530px] max-w-[100vw] h-[calc(100vh-var(--titlebar-height,40px))] bg-[rgb(var(--card))]/95 backdrop-blur-3xl border-l border-[rgba(var(--border),0.12)] shadow-2xl flex flex-col pointer-events-auto overflow-hidden text-[rgb(var(--foreground))]"
          >
            {/* Header Section */}
            <div className="flex items-center justify-between px-6 py-3.5 border-b border-[rgba(var(--border),0.1)] shrink-0 bg-[rgba(var(--foreground),0.02)]">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-2xl bg-purple-500/15 border border-purple-500/30 flex items-center justify-center shrink-0 shadow-md">
                  <Brain size={18} className="text-purple-400" />
                </div>
                <div className="flex flex-col">
                  <h2 className="text-[13px] font-sans font-black tracking-wider uppercase text-[rgb(var(--foreground))]">
                    MEMORY PIPELINE
                  </h2>
                  <span className="text-[10.5px] font-sans text-purple-400/80 font-medium">
                    Live Ingestion Conduit & Telemetry
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={handleTogglePause}
                  title={pipelineProcessingEnabled ? "Pause Pipeline Ingestion" : "Activate Pipeline Ingestion"}
                  className={cn(
                    "p-1.5 rounded-xl transition-all cursor-pointer border flex items-center gap-1.5 px-2.5 text-[10px] font-mono font-bold uppercase shadow-sm",
                    !pipelineProcessingEnabled
                      ? "bg-amber-500/20 text-amber-400 border-amber-500/30 hover:bg-amber-500/30"
                      : "bg-emerald-500/20 text-emerald-400 border-emerald-500/30 hover:bg-emerald-500/30"
                  )}
                >
                  {!pipelineProcessingEnabled ? <Play size={12} /> : <Pause size={12} />}
                  <span>{!pipelineProcessingEnabled ? "PAUSED" : "ACTIVE"}</span>
                </button>
                <button
                  onClick={onRefresh}
                  title="Refresh Pipeline Telemetry"
                  className="p-1.5 rounded-xl text-purple-400 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                >
                  <RefreshCw size={14} className={cn(running && "animate-spin")} />
                </button>
                <button
                  onClick={onClose}
                  className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                  aria-label="Close drawer"
                >
                  <X size={15} />
                </button>
              </div>
            </div>

            {/* Centered Top Navigation Tabs Switcher */}
            <div className="flex items-center justify-center gap-8 px-6 pt-3 pb-2 border-b border-[rgba(var(--border),0.08)] bg-[rgba(var(--foreground),0.01)] text-[12px] font-sans font-bold uppercase tracking-wider shrink-0">
              <button
                onClick={() => setActiveTab("pipeline")}
                className={cn(
                  "transition-all cursor-pointer pb-2 border-b-2 flex items-center gap-2 text-center",
                  activeTab === "pipeline"
                    ? "border-purple-400 text-purple-400 font-black"
                    : "border-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <Activity size={14} />
                <span>Pipeline Flow</span>
              </button>

              <span className="text-[rgb(var(--foreground-muted))]/30 font-light select-none pb-2">|</span>

              <button
                onClick={() => setActiveTab("failed")}
                className={cn(
                  "transition-all cursor-pointer pb-2 border-b-2 flex items-center gap-2 text-center relative",
                  activeTab === "failed"
                    ? "border-red-400 text-red-400 font-black"
                    : "border-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <ShieldAlert size={14} />
                <span>Failed Items</span>
                {failedCount > 0 && (
                  <span className="px-1.5 py-0.2 rounded-full bg-red-500/20 text-red-400 text-[10px] font-mono border border-red-500/30 font-bold">
                    {failedCount}
                  </span>
                )}
              </button>
            </div>

            {/* Scrollable Main Content Area */}
            <div className="flex-1 overflow-y-auto custom-scrollbar p-5 flex flex-col justify-between h-full min-h-0 gap-4">
              {activeTab === "pipeline" ? (
                /* TAB 1: Central Vertical Pipeline Conduit Stream (All 5 Stages) */
                <div className="flex-1 flex flex-col justify-between relative py-2 min-h-[320px] h-full gap-2">
                  {/* Center Glowing Conduit Line */}
                  <div className="absolute left-1/2 top-4 bottom-4 w-[2px] -translate-x-1/2 bg-gradient-to-b from-purple-500 via-blue-500 via-cyan-500 via-amber-500 to-red-500 pointer-events-none opacity-80" />

                  {/* STAGE 1: Left Card | Center Node | Empty Right */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%] flex justify-end">
                      <div className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.1)] hover:border-purple-500/40 transition-all flex flex-col gap-1 shadow-md">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-black tracking-wide text-purple-400 uppercase">
                            1 DEDUPLICATION
                          </span>
                          {stagedPendingCount === 0 ? (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          ) : (
                            <span className="w-2 h-2 rounded-full bg-purple-400 animate-ping" />
                          )}
                        </div>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Exact & sub-word Jaccard dedup
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span className="w-1.5 h-1.5 rounded-full bg-purple-400 animate-pulse" />
                            <span className="w-1.5 h-1.5 rounded-full bg-purple-400/80" />
                            <span className="w-1.5 h-1.5 rounded-full bg-purple-400/60" />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span className="px-2 py-0.5 rounded-lg bg-purple-500/10 border border-purple-500/20 text-purple-400 text-[10px] font-bold">
                            {stagedPendingCount} staged
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Center Node 1 */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border border-purple-500/50 bg-[rgb(var(--card))] text-purple-400 flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(168,85,247,0.35)]">
                      <Filter size={16} />
                    </div>

                    <div className="w-[44%]" />
                  </div>

                  {/* STAGE 2: Empty Left | Center Node | Right Card */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%]" />

                    {/* Center Node 2 */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border border-blue-500/50 bg-[rgb(var(--card))] text-blue-400 flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(59,130,246,0.35)]">
                      <Box size={16} />
                    </div>

                    <div className="w-[44%] flex justify-start">
                      <div className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.1)] hover:border-blue-500/40 transition-all flex flex-col gap-1 shadow-md">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-black tracking-wide text-blue-400 uppercase">
                            2 EMBEDDING
                          </span>
                          {dedupPassCount === 0 ? (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          ) : (
                            <span className="w-2 h-2 rounded-full bg-blue-400 animate-ping" />
                          )}
                        </div>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Generating MiniLM-L12 384d vectors
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
                            <span className="w-1.5 h-1.5 rounded-full bg-blue-400/80" />
                            <span className="w-1.5 h-1.5 rounded-full bg-blue-400/60" />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span className="px-2 py-0.5 rounded-lg bg-blue-500/10 border border-blue-500/20 text-blue-400 text-[10px] font-bold">
                            {dedupPassCount} pending
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* STAGE 3: Left Card | Center Node | Empty Right */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%] flex justify-end">
                      <div className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.1)] hover:border-cyan-500/40 transition-all flex flex-col gap-1 shadow-md">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-black tracking-wide text-cyan-400 uppercase">
                            3 EVALUATION
                          </span>
                          {nliEvaluatedCount === 0 ? (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          ) : (
                            <span className="w-2 h-2 rounded-full bg-cyan-400 animate-ping" />
                          )}
                        </div>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          DeBERTa NLI & ModernBERT Edges
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span className="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse" />
                            <span className="w-1.5 h-1.5 rounded-full bg-cyan-400/80" />
                            <span className="w-1.5 h-1.5 rounded-full bg-cyan-400/60" />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span className="px-2 py-0.5 rounded-lg bg-cyan-500/10 border border-cyan-500/20 text-cyan-400 text-[10px] font-bold">
                            {nliEvaluatedCount} evaluated
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Center Node 3 */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border border-cyan-500/50 bg-[rgb(var(--card))] text-cyan-400 flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(6,182,212,0.35)]">
                      <GitBranch size={16} />
                    </div>

                    <div className="w-[44%]" />
                  </div>

                  {/* STAGE 4: Empty Left | Center Node | Right Card */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%]" />

                    {/* Center Node 4 */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border border-amber-500/50 bg-[rgb(var(--card))] text-amber-400 flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(245,158,11,0.35)]">
                      <Database size={16} />
                    </div>

                    <div className="w-[44%] flex justify-start">
                      <div className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.1)] hover:border-amber-500/40 transition-all flex flex-col gap-1 shadow-md">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-black tracking-wide text-amber-400 uppercase">
                            4 GRAPH COMMIT
                          </span>
                          {running ? (
                            <span className="w-3.5 h-3.5 rounded-full border-2 border-amber-400 border-t-transparent animate-spin shrink-0" />
                          ) : (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          )}
                        </div>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Writing facts to Turso graph DB
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
                            <span className="w-1.5 h-1.5 rounded-full bg-amber-400/80" />
                            <span className="w-1.5 h-1.5 rounded-full bg-amber-400/60" />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span className="px-2 py-0.5 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400 text-[10px] font-bold">
                            {totalPending > 0 ? `${totalPending} ready` : "Synced"}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* STAGE 5: Left Card | Center Node | Empty Right */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%] flex justify-end">
                      <div
                        onClick={() => setActiveTab("failed")}
                        className={cn(
                          "w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border transition-all flex flex-col gap-1 shadow-md cursor-pointer group",
                          failedCount > 0 ? "border-red-500/40 hover:border-red-500/60" : "border-[rgba(var(--border),0.1)] hover:border-[rgba(var(--border),0.2)]"
                        )}
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-black tracking-wide text-red-400 uppercase">
                            5 ATTENTION REQUIRED
                          </span>
                          {failedCount > 0 ? (
                            <AlertCircle size={15} className="text-red-400 shrink-0 animate-pulse" />
                          ) : (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          )}
                        </div>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]">
                          Failed items needing review
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <span className="text-[9.5px] font-mono text-[rgb(var(--foreground-muted))]">
                            {failedCount > 0 ? "Click to view errors" : "No errors"}
                          </span>
                          <span className={cn("px-2 py-0.5 rounded-lg text-[10px] font-bold border", failedCount > 0 ? "bg-red-500/20 text-red-400 border-red-500/30" : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.08)]")}>
                            {failedCount} failed
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Center Node 5 */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border border-red-500/50 bg-[rgb(var(--card))] text-red-400 flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(239,68,68,0.35)]">
                      <ShieldAlert size={16} />
                    </div>

                    <div className="w-[44%]" />
                  </div>
                </div>
              ) : (
                /* TAB 2: Dedicated Failed Queue Items View (Clean Subtle Rows, No Pill Fatigue) */
                <div className="flex-1 flex flex-col justify-between h-full min-h-0 gap-4">
                  <div className="flex items-center justify-between px-1">
                    <span className="text-[11px] font-sans font-bold uppercase tracking-wider text-red-400 flex items-center gap-1.5">
                      <AlertCircle size={14} />
                      Failed Ingestion Items ({failedQueueItems.length})
                    </span>

                    {failedQueueItems.length > 0 && (
                      <button
                        onClick={handleRetryAll}
                        disabled={retrying}
                        className="px-3 py-1.5 rounded-xl bg-[rgb(var(--accent))]/15 hover:bg-[rgb(var(--accent))]/25 border border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] text-[11px] font-sans font-bold transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-40"
                      >
                        <RotateCcw size={12} className={cn(retrying && "animate-spin")} />
                        <span>Retry All Failed</span>
                      </button>
                    )}
                  </div>

                  {failedQueueItems.length === 0 ? (
                    <div className="flex-1 flex flex-col items-center justify-center p-8 text-center bg-[rgba(var(--foreground),0.02)] rounded-2xl border border-[rgba(var(--border),0.06)]">
                      <CheckCircle2 size={32} className="text-emerald-400 mb-2" />
                      <span className="text-[13px] font-sans font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">
                        Zero Failed Items
                      </span>
                      <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 max-w-xs">
                        All queued memory facts processed cleanly without ingestion errors.
                      </span>
                    </div>
                  ) : (
                    <div className="flex-1 flex flex-col gap-2 overflow-y-auto custom-scrollbar pr-1">
                      {failedQueueItems.map((item) => (
                        <div key={item.id} className="p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] transition-all flex flex-col gap-1.5">
                          <div className="flex items-start justify-between gap-3">
                            <span className="text-[12px] font-sans text-[rgb(var(--foreground))] font-medium">
                              "{item.fact}"
                            </span>
                            <button
                              onClick={() => handleRetrySingleItem(item.id)}
                              title="Retry this item"
                              className="px-2.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.05)] hover:bg-[rgba(var(--foreground),0.1)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] text-[10.5px] font-sans font-medium transition-colors cursor-pointer shrink-0 border border-[rgba(var(--border),0.1)] flex items-center gap-1"
                            >
                              <RotateCcw size={10} />
                              <span>Retry</span>
                            </button>
                          </div>

                          <div className="flex items-center justify-between text-[10.5px] font-sans text-[rgb(var(--foreground-muted))] pt-1 border-t border-[rgba(var(--border),0.06)]">
                            <span>Collection: <strong className="text-purple-400 font-semibold">{item.collection}</strong></span>
                            <span>Attempts: <strong className="text-amber-400/80 font-semibold">{item.attempts}</strong></span>
                          </div>

                          {item.error_msg && (
                            <span className="text-[10.5px] font-sans text-red-400/80 line-clamp-2">
                              Error: {item.error_msg}
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  )}

                  {lastRetriedCount !== null && (
                    <div className="p-2.5 rounded-xl bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 text-[11px] font-sans font-medium text-center flex items-center justify-center gap-1.5">
                      <Check size={14} />
                      <span>Re-queued {lastRetriedCount} failed items for processing sweep.</span>
                    </div>
                  )}
                </div>
              )}

              {/* Expanded Prominent Bottom Telemetry Strip */}
              <div className="p-3.5 rounded-2xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.1)] grid grid-cols-4 gap-2 shrink-0 shadow-lg text-center">
                <div className="flex flex-col items-center">
                  <TrendingUp size={16} className="text-purple-400 mb-0.5" />
                  <span className="text-[14px] font-sans font-black text-[rgb(var(--foreground))]">
                    {activeNodesCount.toLocaleString()}
                  </span>
                  <span className="text-[9px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Graph Nodes
                  </span>
                </div>

                <div className="flex flex-col items-center border-l border-[rgba(var(--border),0.08)]">
                  <Layers size={16} className="text-blue-400 mb-0.5" />
                  <span className="text-[14px] font-sans font-black text-[rgb(var(--foreground))]">
                    {activeEdgesCount.toLocaleString()}
                  </span>
                  <span className="text-[9px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Graph Edges
                  </span>
                </div>

                <div className="flex flex-col items-center border-l border-[rgba(var(--border),0.08)]">
                  <Clock size={16} className="text-cyan-400 mb-0.5" />
                  <span className="text-[14px] font-sans font-black text-amber-400">
                    {totalPending}
                  </span>
                  <span className="text-[9px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Queue Items
                  </span>
                </div>

                <div className="flex flex-col items-center border-l border-[rgba(var(--border),0.08)]">
                  <ShieldCheck size={16} className={cn("mb-0.5", failedCount > 0 ? "text-red-400" : "text-emerald-400")} />
                  <span className={cn("text-[14px] font-sans font-black", failedCount > 0 ? "text-red-400" : "text-emerald-400")}>
                    {failedCount > 0 ? `${failedCount} Failed` : "Healthy"}
                  </span>
                  <span className="text-[9px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Queue Health
                  </span>
                </div>
              </div>
            </div>

            {/* Bottom Immediate Queue Processing Control */}
            <div className="p-4 border-t border-[rgba(var(--border),0.1)] bg-[rgba(var(--foreground),0.02)] shrink-0 flex flex-col gap-2">
              {lastProcessedCount !== null && (
                <div className="flex items-center gap-1.5 text-emerald-400 text-[11px] font-sans justify-center font-medium">
                  <CheckCircle2 size={14} />
                  <span>Swept & consolidated {lastProcessedCount} queued items into memory graph.</span>
                </div>
              )}

              <button
                onClick={handleTrigger}
                disabled={running}
                className="w-full py-3.5 rounded-xl bg-purple-600/20 hover:bg-purple-600/30 border border-purple-500/40 text-purple-400 hover:text-[rgb(var(--foreground))] transition-all flex items-center justify-center gap-2 cursor-pointer disabled:opacity-40 text-[12.5px] font-sans font-bold uppercase tracking-wider shadow-lg"
              >
                {running ? (
                  <Sparkles size={16} className="animate-spin text-purple-400" />
                ) : (
                  <Zap size={16} className="text-purple-400" />
                )}
                <span>{running ? MEMORY_COPY.consolidating : "PROCESS PENDING QUEUE"}</span>
              </button>
              <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))] text-center">
                Runs immediate pipeline sweep over uncommitted queued facts
              </span>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
});

MemoryPipelineDrawer.displayName = "MemoryPipelineDrawer";
