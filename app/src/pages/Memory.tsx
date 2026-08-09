import React, { useState, useEffect, useCallback, useRef } from "react";
import { Search, X, Cpu } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { AmbientBackground } from "@/shared/components/common";
import { MemoryGraph } from "@/shared/components/memory/MemoryGraph";
import { MemoryLegendCard } from "@/shared/components/memory/MemoryLegendCard";
import { MemoryNodeTooltip } from "@/shared/components/memory/MemoryNodeTooltip";
import { PipelineMonitorPopover } from "@/shared/components/memory/PipelineMonitor";
import {
  MemoryFactEntry,
  MemoryRelationEntry,
  MemoryQueueSummary,
  getMemoryGraph,
  getMemoryRelations,
  getMemoryQueueStatus,
} from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

export const Memory: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerBtnRef = useRef<HTMLButtonElement>(null);
  const [dims, setDims] = useState({ w: 0, h: 0 });

  const [facts, setFacts] = useState<MemoryFactEntry[]>([]);
  const [relations, setRelations] = useState<MemoryRelationEntry[]>([]);
  const [queueSummary, setQueueSummary] = useState<MemoryQueueSummary | null>(null);

  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCollection, setSelectedCollection] = useState<string>("all");
  const [selectedRelation, setSelectedRelation] = useState<string>("all");
  const [selectedFact, setSelectedFact] = useState<MemoryFactEntry | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);
  const [pipelineOpen, setPipelineOpen] = useState(false);

  // Measure container
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const obs = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setDims({ w: entry.contentRect.width, h: entry.contentRect.height });
      }
    });
    obs.observe(el);
    setDims({ w: el.clientWidth, h: el.clientHeight });
    return () => obs.disconnect();
  }, []);

  const loadData = useCallback(async () => {
    try {
      const [fData, rData, qData] = await Promise.all([
        getMemoryGraph(),
        getMemoryRelations(),
        getMemoryQueueStatus(),
      ]);
      setFacts(fData);
      setRelations(rData);
      setQueueSummary(qData);
    } catch (e) {
      console.error("Memory data load failed:", e);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleSelectFact = useCallback((fact: MemoryFactEntry | null, clickPos?: { x: number; y: number }) => {
    setSelectedFact(fact);
    if (fact && clickPos) {
      setTooltipPos(clickPos);
    } else {
      setTooltipPos(null);
    }
  }, []);

  const totalPending = queueSummary
    ? (queueSummary.staged_pending ?? 0) +
      (queueSummary.dedup_pass ?? 0) +
      (queueSummary.nli_evaluated ?? 0) +
      (queueSummary.paused ?? 0)
    : 0;

  return (
    <div className="flex-1 relative overflow-hidden select-none w-full h-full bg-[rgb(var(--background))]">
      {/* Ambient background effect directly on page */}
      <AmbientBackground mood="calm" originX="50%" originY="50%" />

      {/* Main graph canvas directly in main div on top of background */}
      <div ref={containerRef} className="absolute inset-0 z-10">
        {dims.w > 0 && (
          <MemoryGraph
            facts={facts}
            relations={relations}
            width={dims.w}
            height={dims.h}
            searchQuery={searchQuery}
            selectedCollection={selectedCollection}
            selectedRelation={selectedRelation}
            onSelectFact={(fact, pos) => handleSelectFact(fact, pos)}
            selectedFactId={selectedFact?.id ?? null}
          />
        )}
      </div>

      {/* Top-Center: Decoupled Search Pill Bar */}
      <div className="absolute top-4 left-1/2 -translate-x-1/2 z-20 pointer-events-auto">
        <div className="glass-card h-[42px] px-4 rounded-full border border-[rgba(var(--accent),0.18)] bg-[rgba(10,12,14,0.65)] backdrop-blur-xl flex items-center gap-2.5 shadow-2xl w-[320px] focus-within:w-[420px] transition-all duration-300">
          <Search size={14} className="text-[rgb(var(--accent))] shrink-0" />
          <input
            type="text"
            placeholder="Search memory facts..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full bg-transparent text-[12px] font-mono tracking-wide text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 outline-none"
          />
          <AnimatePresence>
            {searchQuery && (
              <motion.button
                initial={{ opacity: 0, scale: 0.8 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.8 }}
                onClick={() => setSearchQuery("")}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              >
                <X size={12} />
              </motion.button>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* Top-Left: Two-Column Collections & Relations Legend Card */}
      <div className="absolute top-4 left-4 z-20 pointer-events-auto">
        <MemoryLegendCard
          selectedCollection={selectedCollection}
          onSelectCollection={setSelectedCollection}
          selectedRelation={selectedRelation}
          onSelectRelation={setSelectedRelation}
        />
      </div>

      {/* Floating Memory Node Tooltip with Connected Edges Details */}
      <MemoryNodeTooltip
        fact={selectedFact}
        allFacts={facts}
        allRelations={relations}
        pos={tooltipPos}
        onClose={() => handleSelectFact(null)}
        onRefresh={loadData}
      />

      {/* Bottom-Right: Floating Pipeline Monitor Trigger Button */}
      <div className="fixed bottom-4 right-4 z-50 pointer-events-auto">
        <div className="relative group">
          <button
            ref={triggerBtnRef}
            onClick={() => setPipelineOpen((v) => !v)}
            className={cn(
              "flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 cursor-pointer glass-card",
              pipelineOpen
                ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/60 shadow-[0_0_15px_rgba(var(--accent),0.3)]"
                : "bg-transparent border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
            )}
            aria-label="Pipeline Monitor"
          >
            <Cpu size={20} className={cn(totalPending > 0 && "animate-pulse")} />
            {totalPending > 0 && (
              <span className="absolute -top-0.5 -right-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-[rgb(var(--accent))] text-[9px] font-mono font-black text-black">
                {totalPending}
              </span>
            )}
          </button>

          {/* Tooltip */}
          <span className="absolute bottom-14 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none px-2.5 py-1 rounded-md text-[11px] font-bold tracking-wider uppercase bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-lg whitespace-nowrap">
            Pipeline Monitor
          </span>
        </div>
      </div>

      {/* Bottom-Right: Pipeline Monitor Popover */}
      <PipelineMonitorPopover
        summary={queueSummary}
        onRefresh={loadData}
        open={pipelineOpen}
        onClose={() => setPipelineOpen(false)}
      />
    </div>
  );
};
