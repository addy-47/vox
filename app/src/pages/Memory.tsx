import React, { useState, useEffect, useCallback, useRef } from "react";
import { Search, X, SlidersHorizontal, Focus, Cpu } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { AmbientBackground } from "@/shared/components/common";
import { MemoryGraph, MemoryGraphRef, COLLECTION_COLORS } from "@/shared/components/memory/MemoryGraph";
import { MemoryLegendCard } from "@/shared/components/memory/MemoryLegendCard";
import { MemoryNodeTooltip } from "@/shared/components/memory/MemoryNodeTooltip";
import { MemoryPipelineDrawer } from "@/shared/components/memory/MemoryPipelineDrawer";
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
  const graphRef = useRef<MemoryGraphRef>(null);
  const filterBtnRef = useRef<HTMLButtonElement>(null);

  const [dims, setDims] = useState({ w: 0, h: 0 });
  const [facts, setFacts] = useState<MemoryFactEntry[]>([]);
  const [relations, setRelations] = useState<MemoryRelationEntry[]>([]);
  const [queueSummary, setQueueSummary] = useState<MemoryQueueSummary | null>(null);

  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCollection, setSelectedCollection] = useState<string>("all");
  const [selectedRelation, setSelectedRelation] = useState<string>("all");
  const [selectedFact, setSelectedFact] = useState<MemoryFactEntry | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [filterMenuOpen, setFilterMenuOpen] = useState(false);

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
            ref={graphRef}
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

      {/* Top-Center: Decoupled Search Pill Bar with Filter Popover */}
      <div className="absolute top-4 left-1/2 -translate-x-1/2 z-20 pointer-events-auto flex items-center gap-2">
        <div className="glass-card h-[42px] px-3.5 rounded-full border border-[rgba(var(--accent),0.18)] bg-[rgba(10,12,14,0.70)] backdrop-blur-xl flex items-center gap-2.5 shadow-2xl w-[320px] focus-within:w-[400px] transition-all duration-300">
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
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0"
              >
                <X size={12} />
              </motion.button>
            )}
          </AnimatePresence>

          <div className="h-4 w-px bg-white/10 shrink-0" />

          {/* Filter Popover Toggle Button */}
          <div className="relative shrink-0">
            <button
              ref={filterBtnRef}
              onClick={() => setFilterMenuOpen((v) => !v)}
              className={cn(
                "p-1.5 rounded-full text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] transition-colors cursor-pointer",
                (filterMenuOpen || selectedCollection !== "all" || selectedRelation !== "all") &&
                  "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10"
              )}
              title="Filter by Collection or Status"
            >
              <SlidersHorizontal size={13} />
            </button>

            {/* Quick Filter Dropdown */}
            <AnimatePresence>
              {filterMenuOpen && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.95, y: 6 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.95, y: 6 }}
                  transition={{ duration: 0.15 }}
                  className="absolute right-0 top-10 w-[200px] glass-card p-3 rounded-2xl border border-[rgba(var(--accent),0.15)] bg-[rgba(10,12,14,0.92)] backdrop-blur-2xl shadow-2xl flex flex-col gap-2 z-30"
                >
                  <div className="flex items-center justify-between border-b border-white/[0.06] pb-1.5">
                    <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
                      Quick Filters
                    </span>
                    {(selectedCollection !== "all" || selectedRelation !== "all") && (
                      <button
                        onClick={() => {
                          setSelectedCollection("all");
                          setSelectedRelation("all");
                        }}
                        className="text-[9px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
                      >
                        RESET
                      </button>
                    )}
                  </div>

                  <div className="flex flex-col gap-1">
                    {["all", "Identity", "Profile", "Directives", "Constraints", "Entities", "Inactive"].map(
                      (col) => {
                        const isSelected = selectedCollection === col;
                        const colStyle = (COLLECTION_COLORS as any)[col];
                        return (
                          <button
                            key={col}
                            onClick={() => {
                              setSelectedCollection(selectedCollection === col ? "all" : col);
                            }}
                            className={cn(
                              "flex items-center gap-2 px-2 py-1 rounded-lg text-[10px] font-mono text-left transition-colors cursor-pointer",
                              isSelected
                                ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] font-bold"
                                : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-white/[0.04]"
                            )}
                          >
                            <span
                              className="w-2 h-2 rounded-full shrink-0"
                              style={{ backgroundColor: colStyle ? colStyle.main : "rgb(var(--accent))" }}
                            />
                            <span>{col === "all" ? "ALL COLLECTIONS" : col.toUpperCase()}</span>
                          </button>
                        );
                      }
                    )}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </div>

      {/* Top-Left: Collapsible Two-Column Collections & Relations Legend Card */}
      <div className="absolute top-4 left-4 z-20 pointer-events-auto">
        <MemoryLegendCard
          selectedCollection={selectedCollection}
          onSelectCollection={setSelectedCollection}
          selectedRelation={selectedRelation}
          onSelectRelation={setSelectedRelation}
          totalFactsCount={facts.length}
          totalRelationsCount={relations.length}
        />
      </div>

      {/* Top-Right: Recenter Graph Camera Button */}
      <div className="absolute top-4 right-4 z-20 pointer-events-auto">
        <button
          onClick={() => graphRef.current?.recenter()}
          className="flex items-center gap-1.5 px-3 py-2 rounded-full glass-card border border-[rgba(var(--accent),0.15)] bg-[rgba(10,12,14,0.70)] backdrop-blur-xl text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-all cursor-pointer shadow-xl text-[11px] font-mono font-bold uppercase tracking-wider"
          title="Recenter Graph View"
        >
          <Focus size={13} className="text-[rgb(var(--accent))]" />
          <span>Recenter</span>
        </button>
      </div>

      {/* Bottom Right Edge Nav: Pipeline Processing Center Trigger Pill */}
      <div className="fixed bottom-4 right-4 z-30 pointer-events-auto">
        <button
          onClick={() => setDrawerOpen(true)}
          className={cn(
            "flex items-center gap-2.5 px-3.5 py-2.5 rounded-full glass-card border border-[rgba(var(--accent),0.25)] bg-[rgba(10,12,14,0.85)] hover:bg-[rgba(10,12,14,0.95)] backdrop-blur-xl text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.5)] transition-all cursor-pointer shadow-2xl group",
            drawerOpen && "opacity-0 pointer-events-none"
          )}
          title="Open Memory Processing Center"
        >
          <div className="relative">
            <Cpu size={16} className="text-[rgb(var(--accent))] group-hover:scale-110 transition-transform" />
            {totalPending > 0 && (
              <span className="absolute -top-1 -right-1 flex h-3 w-3 items-center justify-center rounded-full bg-[rgb(var(--accent))] text-[8px] font-mono font-black text-black animate-pulse">
                {totalPending}
              </span>
            )}
          </div>
          <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground))]/90">
            Pipeline {totalPending > 0 ? `(${totalPending})` : ""}
          </span>
        </button>
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

      {/* Right Slide-Out Pipeline Observability Drawer */}
      <MemoryPipelineDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        summary={queueSummary}
        facts={facts}
        relations={relations}
        onRefresh={loadData}
      />
    </div>
  );
};
