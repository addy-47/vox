import React, { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Cpu, Play, X, RefreshCw, AlertCircle } from "lucide-react";
import { MemoryQueueSummary, triggerMemoryConsolidation } from "@/services/memoryService";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";
import { cn } from "@/shared/lib/utils";

interface PipelineMonitorProps {
  summary: MemoryQueueSummary | null;
  onRefresh: () => void;
  open: boolean;
  onClose: () => void;
}

const STAGES = [
  { key: "staged_pending", label: "DEDUP", desc: "Exact Duplicate Stage" },
  { key: "dedup_pass", label: "EMBED", desc: "Embedding Vector Stage" },
  { key: "nli_evaluated", label: "NLI EVAL", desc: "DeBERTa-v3 Conflict Stage" },
  { key: "paused", label: "COMMIT", desc: "Turso SQLite Storage" },
] as const;

function formatElapsed(created_at: number): string {
  const diffSec = Math.max(0, Math.floor((Date.now() - created_at) / 1000));
  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  return `${Math.floor(diffMin / 60)}h ago`;
}

export const PipelineMonitorPopover: React.FC<PipelineMonitorProps> = ({
  summary,
  onRefresh,
  open,
  onClose,
}) => {
  const [running, setRunning] = useState(false);
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot | null>(null);

  // Direct DOM Refs for 60fps smooth EMA resource bars
  const cpuTextRef = useRef<HTMLSpanElement>(null);
  const cpuBarRef = useRef<HTMLDivElement>(null);
  const ramTextRef = useRef<HTMLSpanElement>(null);
  const ramBarRef = useRef<HTMLDivElement>(null);

  const totalPending = summary
    ? (summary.staged_pending ?? 0) +
      (summary.dedup_pass ?? 0) +
      (summary.nli_evaluated ?? 0) +
      (summary.paused ?? 0)
    : 0;

  // Poll runtime telemetry at 1Hz when open
  useEffect(() => {
    if (!open) return;

    const pollTelemetry = async () => {
      try {
        const snap = await getRuntimeSnapshot();
        if (snap) setSnapshot(snap);
      } catch {
        // silent
      }
    };

    pollTelemetry();
    const intervalId = setInterval(pollTelemetry, 1000);
    return () => clearInterval(intervalId);
  }, [open]);

  // Smooth interpolation loop for CPU & RAM indicators
  useEffect(() => {
    if (!open) return;

    let curCpu = 0;
    let curRam = 0;
    let rafId = 0;

    const tick = () => {
      if (snapshot) {
        curCpu += (snapshot.vox_cpu_usage - curCpu) * 0.15;
        curRam += (snapshot.vox_ram_mb - curRam) * 0.15;

        if (cpuTextRef.current) {
          cpuTextRef.current.textContent = `${curCpu.toFixed(1)}%`;
        }
        if (cpuBarRef.current) {
          cpuBarRef.current.style.width = `${Math.min(100, Math.max(0, curCpu))}%`;
        }
        if (ramTextRef.current) {
          ramTextRef.current.textContent = `${(curRam / 1024).toFixed(2)} GB`;
        }
        if (ramBarRef.current) {
          const total = snapshot.total_ram_mb || 8192;
          const pct = Math.min(100, Math.max(0, (curRam / total) * 100));
          ramBarRef.current.style.width = `${pct}%`;
        }
      }
      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [open, snapshot]);

  const handleTrigger = async () => {
    setRunning(true);
    try {
      await triggerMemoryConsolidation();
      setTimeout(onRefresh, 600);
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
          initial={{ opacity: 0, scale: 0.95, y: 10 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, y: 10 }}
          transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
          className="fixed z-[200] bottom-[72px] right-4 w-[330px] glass-card p-4 rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgba(10,12,14,0.92)] backdrop-blur-2xl shadow-2xl flex flex-col gap-3.5 pointer-events-auto"
          role="dialog"
          aria-label="Pipeline Queue"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-white/[0.06] pb-2.5">
            <div className="flex items-center gap-2">
              <Cpu size={15} className="text-[rgb(var(--accent))]" />
              <span className="text-[11px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--foreground))]">
                PIPELINE TELEMETRY
              </span>
            </div>
            <div className="flex items-center gap-1.5">
              <button
                onClick={handleTrigger}
                disabled={running}
                title="Trigger Immediate Consolidation Cycle"
                className={cn(
                  "p-1 rounded-lg text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer",
                  running && "animate-spin opacity-50"
                )}
              >
                <RefreshCw size={13} />
              </button>
              <button
                onClick={onClose}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                aria-label="Close"
              >
                <X size={14} />
              </button>
            </div>
          </div>

          {/* Live System Resource Meters */}
          <div className="grid grid-cols-2 gap-2.5 p-2.5 rounded-xl bg-white/[0.02] border border-white/[0.04]">
            {/* CPU Meter */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between items-baseline">
                <span className="text-[9px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60">
                  PIPELINE CPU
                </span>
                <span ref={cpuTextRef} className="text-[11px] font-mono font-bold text-[rgb(var(--foreground))]">
                  0.0%
                </span>
              </div>
              <div className="h-[3px] w-full rounded-full bg-white/[0.06] overflow-hidden">
                <div ref={cpuBarRef} className="h-full rounded-full bg-[rgb(var(--accent))] transition-all duration-300" style={{ width: "0%" }} />
              </div>
            </div>

            {/* RAM Meter */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between items-baseline">
                <span className="text-[9px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60">
                  MEMORY RAM
                </span>
                <span ref={ramTextRef} className="text-[11px] font-mono font-bold text-[rgb(var(--foreground))]">
                  0.00 GB
                </span>
              </div>
              <div className="h-[3px] w-full rounded-full bg-white/[0.06] overflow-hidden">
                <div ref={ramBarRef} className="h-full rounded-full bg-[rgb(var(--accent))] transition-all duration-300" style={{ width: "0%" }} />
              </div>
            </div>
          </div>

          {/* Pipeline Stages Queue Breakdown */}
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-[10px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/70">
                Ingestion Queue Stages
              </span>
              <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
                {totalPending} Total In Queue
              </span>
            </div>

            <div className="grid grid-cols-2 gap-1.5">
              {STAGES.map((stage) => {
                const count = summary ? (summary as any)[stage.key] ?? 0 : 0;
                return (
                  <div
                    key={stage.key}
                    className="flex items-center justify-between p-2 rounded-xl bg-white/[0.02] border border-white/[0.04]"
                  >
                    <div className="flex items-center gap-1.5 min-w-0">
                      <span
                        className={cn(
                          "w-1.5 h-1.5 rounded-full shrink-0",
                          count > 0 ? "bg-[rgb(var(--accent))] animate-pulse" : "bg-white/15"
                        )}
                      />
                      <span className="text-[10px] font-mono font-bold tracking-wider text-[rgb(var(--foreground))]/80 truncate">
                        {stage.label}
                      </span>
                    </div>
                    <span
                      className={cn(
                        "text-[11px] font-mono font-bold shrink-0",
                        count > 0 ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/30"
                      )}
                    >
                      {count}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Live Recent Items Stream */}
          {summary?.recent_items && summary.recent_items.length > 0 && (
            <div className="flex flex-col gap-1.5 border-t border-white/[0.06] pt-2">
              <span className="text-[9px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/50">
                Live Queue Stream
              </span>
              <div className="flex flex-col gap-1 max-h-[85px] overflow-y-auto custom-scrollbar pr-1">
                {summary.recent_items.slice(0, 4).map((item) => (
                  <div
                    key={item.id}
                    className="flex items-center justify-between gap-2 p-1.5 rounded-lg bg-white/[0.02] border border-white/[0.03] text-[10px]"
                  >
                    <span className="font-mono text-[rgb(var(--accent))]/80 font-bold uppercase text-[9px] shrink-0">
                      {item.status.replace("_", " ")}
                    </span>
                    <span className="text-[11px] font-light text-[rgb(var(--foreground))]/70 truncate flex-1">
                      {item.fact}
                    </span>
                    <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40 shrink-0">
                      {formatElapsed(item.created_at)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Error Banner if any failed items */}
          {summary?.failed !== undefined && summary.failed > 0 && (
            <div className="flex items-center justify-between p-2 rounded-xl bg-red-500/10 border border-red-500/20 text-red-400 text-[10px] font-mono">
              <div className="flex items-center gap-1.5">
                <AlertCircle size={12} />
                <span className="font-bold uppercase">Failed Items</span>
              </div>
              <span className="font-bold">{summary.failed}</span>
            </div>
          )}

          {/* Trigger Action Button */}
          <button
            onClick={handleTrigger}
            disabled={running}
            className="w-full py-2 rounded-xl border border-[rgba(var(--accent),0.3)] bg-[rgba(var(--accent),0.1)] hover:bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] transition-all flex items-center justify-center gap-2 text-[11px] font-mono font-bold tracking-widest uppercase cursor-pointer disabled:opacity-40"
          >
            <Play size={11} fill="currentColor" />
            {running ? "Consolidating Queue..." : "TRIGGER CONSOLIDATION CYCLE"}
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
