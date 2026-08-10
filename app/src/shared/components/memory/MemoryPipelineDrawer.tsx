import React, { useState, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Cpu,
  X,
  RefreshCw,
  Database,
  Layers,
  AlertCircle,
  CheckCircle2,
  Sparkles,
  Zap,
} from "lucide-react";
import {
  MemoryFactEntry,
  MemoryRelationEntry,
  MemoryQueueSummary,
  triggerMemoryConsolidation,
} from "@/services/memoryService";
import { COLLECTION_COLORS } from "@/shared/components/memory/MemoryGraph";
import { cn } from "@/shared/lib/utils";

interface MemoryPipelineDrawerProps {
  open: boolean;
  onClose: () => void;
  summary: MemoryQueueSummary | null;
  facts: MemoryFactEntry[];
  relations: MemoryRelationEntry[];
  onRefresh: () => void;
}

const STAGES = [
  {
    key: "staged_pending",
    title: "1. Deduplication",
    badge: "Stage 1",
    desc: "Filters out verbatim and near-exact duplicate facts",
  },
  {
    key: "dedup_pass",
    title: "2. Vector Embedding",
    badge: "Stage 2",
    desc: "Generates semantic vector embeddings for fast retrieval",
  },
  {
    key: "nli_evaluated",
    title: "3. Fact Reasoning",
    badge: "Stage 3",
    desc: "Evaluates fact updates and graph relationships",
  },
  {
    key: "paused",
    title: "4. Knowledge Storage",
    badge: "Stage 4",
    desc: "Saves verified facts into long-term memory store",
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
  facts,
  relations,
  onRefresh,
}) => {
  const [running, setRunning] = useState(false);
  const [lastProcessedCount, setLastProcessedCount] = useState<number | null>(null);

  const totalPending = summary
    ? (summary.staged_pending ?? 0) +
      (summary.dedup_pass ?? 0) +
      (summary.nli_evaluated ?? 0) +
      (summary.paused ?? 0)
    : 0;

  // Breakdown of facts per collection
  const collectionCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    facts.forEach((f) => {
      const col = f.collection || "Identity";
      counts[col] = (counts[col] || 0) + 1;
    });
    return counts;
  }, [facts]);

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
            className="fixed right-4 bottom-4 z-50 w-[420px] max-w-[94vw] max-h-[85vh] glass-card border border-[rgba(var(--accent),0.2)] bg-[rgba(10,12,14,0.95)] backdrop-blur-2xl shadow-[0_20px_60px_rgba(0,0,0,0.6)] rounded-3xl flex flex-col pointer-events-auto overflow-hidden"
          >
            {/* Panel Header */}
            <div className="flex items-center justify-between px-5 py-4 border-b border-white/[0.08] bg-white/[0.01]">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center shrink-0">
                  <Cpu size={16} className="text-[rgb(var(--accent))]" />
                </div>
                <div>
                  <h2 className="text-[13px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground))]">
                    Memory Processing Center
                  </h2>
                  <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/70">
                    Live Ingestion & Health Status
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-1.5">
                <button
                  onClick={onRefresh}
                  title="Refresh Memory Status"
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
              {/* Live Status Card */}
              <div className="p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.07] flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div
                    className={cn(
                      "w-3 h-3 rounded-full shrink-0",
                      totalPending > 0 ? "bg-[rgb(var(--accent))] animate-pulse" : "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.5)]"
                    )}
                  />
                  <div>
                    <span className="text-[12px] font-sans font-semibold text-[rgb(var(--foreground))] block">
                      {totalPending > 0
                        ? `${totalPending} ${totalPending === 1 ? "Fact" : "Facts"} Queued for Ingestion`
                        : "Memory Ingestion Synchronized"}
                    </span>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60">
                      {totalPending > 0
                        ? "Auto-consolidating background queue..."
                        : "Ready · Idle background daemon"}
                    </span>
                  </div>
                </div>

                <span
                  className={cn(
                    "px-2.5 py-1 rounded-full text-[9px] font-mono font-bold tracking-wider uppercase shrink-0",
                    totalPending > 0
                      ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30"
                      : "bg-emerald-500/15 text-emerald-400 border border-emerald-500/30"
                  )}
                >
                  {totalPending > 0 ? "PROCESSING" : "READY"}
                </span>
              </div>

              {/* 4 Ingestion Pipeline Stages */}
              <div className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/80">
                    Ingestion Stages
                  </span>
                  <span className="text-[10px] font-mono text-[rgb(var(--accent))] font-medium">
                    4-Stage Pipeline
                  </span>
                </div>

                <div className="grid grid-cols-2 gap-2">
                  {STAGES.map((stage) => {
                    const count = summary ? (summary as any)[stage.key] ?? 0 : 0;
                    return (
                      <div
                        key={stage.key}
                        className="p-3 rounded-2xl bg-white/[0.02] border border-white/[0.05] hover:border-[rgba(var(--accent),0.2)] transition-all flex flex-col justify-between gap-1.5"
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-bold text-[rgb(var(--foreground))]">
                            {stage.title}
                          </span>
                          <span
                            className={cn(
                              "text-[12px] font-mono font-bold",
                              count > 0 ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/30"
                            )}
                          >
                            {count}
                          </span>
                        </div>
                        <span className="text-[10px] font-sans text-[rgb(var(--foreground-muted))]/60 leading-normal">
                          {stage.desc}
                        </span>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Knowledge Base Overview */}
              <div className="p-3.5 rounded-2xl bg-white/[0.02] border border-white/[0.05] space-y-3">
                <div className="flex items-center justify-between border-b border-white/[0.06] pb-2">
                  <div className="flex items-center gap-2">
                    <Database size={14} className="text-[rgb(var(--accent))]" />
                    <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground))]">
                      Knowledge Base Totals
                    </span>
                  </div>
                  <div className="flex items-center gap-3 text-[11px] font-mono font-bold">
                    <span className="text-[rgb(var(--accent))]">{facts.length.toLocaleString()} Facts</span>
                    <span className="text-white/20">|</span>
                    <span className="text-emerald-400">{relations.length} Connections</span>
                  </div>
                </div>

                {/* Collection Distribution */}
                <div className="grid grid-cols-3 gap-2 pt-1">
                  {Object.entries(COLLECTION_COLORS)
                    .filter(([k]) => k !== "Inactive")
                    .map(([col, style]) => {
                      const count = collectionCounts[col] || 0;
                      return (
                        <div
                          key={col}
                          className="flex items-center justify-between px-2 py-1 rounded-xl bg-white/[0.02] border border-white/[0.04] text-[10px]"
                        >
                          <div className="flex items-center gap-1.5 truncate">
                            <span
                              className="w-2 h-2 rounded-full shrink-0"
                              style={{ backgroundColor: style.main }}
                            />
                            <span className="font-sans text-[rgb(var(--foreground-muted))]/80 truncate">
                              {col}
                            </span>
                          </div>
                          <span className="font-mono font-bold text-[rgb(var(--foreground))] shrink-0">
                            {count}
                          </span>
                        </div>
                      );
                    })}
                </div>
              </div>

              {/* Live Queue Items Stream */}
              <div className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <div className="flex items-center gap-1.5">
                    <Layers size={13} className="text-[rgb(var(--accent))]" />
                    <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/80">
                      Recent Ingestion Activity
                    </span>
                  </div>
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50">
                    Live Stream
                  </span>
                </div>

                {summary?.recent_items && summary.recent_items.length > 0 ? (
                  <div className="space-y-2 max-h-[180px] overflow-y-auto custom-scrollbar pr-1">
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
                      Queue is clean · All facts processed
                    </span>
                  </div>
                )}
              </div>

              {/* Failed Items Alert */}
              {summary?.failed !== undefined && summary.failed > 0 && (
                <div className="p-3 rounded-2xl bg-red-500/10 border border-red-500/20 text-red-400 flex items-center justify-between text-[11px] font-mono">
                  <div className="flex items-center gap-2">
                    <AlertCircle size={14} />
                    <span className="font-bold uppercase tracking-wider">Failed Queue Items</span>
                  </div>
                  <span className="font-bold">{summary.failed}</span>
                </div>
              )}
            </div>

            {/* Footer Control Panel */}
            <div className="p-4 border-t border-white/[0.08] bg-white/[0.01] flex flex-col gap-2">
              {lastProcessedCount !== null && (
                <div className="flex items-center gap-1.5 text-emerald-400 text-[11px] font-mono justify-center">
                  <CheckCircle2 size={13} />
                  <span>Consolidated {lastProcessedCount} facts to memory database.</span>
                </div>
              )}
              <button
                onClick={handleTrigger}
                disabled={running}
                className="w-full py-2.5 rounded-2xl border border-[rgba(var(--accent),0.35)] bg-[rgba(var(--accent),0.12)] hover:bg-[rgba(var(--accent),0.22)] text-[rgb(var(--accent))] transition-all flex items-center justify-center gap-2 text-[11px] font-mono font-bold tracking-widest uppercase cursor-pointer disabled:opacity-40 shadow-lg"
              >
                {running ? (
                  <Sparkles size={14} className="animate-spin text-[rgb(var(--accent))]" />
                ) : (
                  <Zap size={14} className="text-[rgb(var(--accent))]" />
                )}
                {running ? "Processing Memory Queue..." : "Run Memory Consolidation Now"}
              </button>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
};

