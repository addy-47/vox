import React, { useState, useEffect, useCallback, useRef } from "react";
import { Search, X, SlidersHorizontal, Focus, Cpu, AlertTriangle, ShieldAlert } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { AmbientBackground } from "@/shared/components/common";
import { MemoryGraph, MemoryGraphRef, COLLECTION_COLORS } from "@/shared/components/memory/MemoryGraph";
import { MemoryLegendCard } from "@/shared/components/memory/MemoryLegendCard";
import { MemoryMetricsCard } from "@/shared/components/memory/MemoryMetricsCard";
import { MemoryNodeTooltip } from "@/shared/components/memory/MemoryNodeTooltip";
import { MemoryPipelineDrawer } from "@/shared/components/memory/MemoryPipelineDrawer";
import {
  MemoryNodeTopology,
  MemoryEdgeTopology,
  MemoryFactDetail,
  MemoryQueueSummary,
  MemoryConflict,
  getGraphVersion,
  getMemoryGraphTopology,
  getMemoryFactDetail,
  getMemoryQueueStatus,
  getUnresolvedConflicts,
  resolveMemoryConflict,
} from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

export const Memory: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<MemoryGraphRef>(null);
  const filterBtnRef = useRef<HTMLButtonElement>(null);

  const [dims, setDims] = useState({ w: 0, h: 0 });

  // Topology data cache
  const [nodes, setNodes] = useState<MemoryNodeTopology[]>([]);
  const [edges, setEdges] = useState<MemoryEdgeTopology[]>([]);
  const [lastVersion, setLastVersion] = useState<number>(-1);

  // Lazy loaded detail for selected node
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedFactDetail, setSelectedFactDetail] = useState<MemoryFactDetail | null>(null);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);

  // Ingestion Queue Status
  const [queueSummary, setQueueSummary] = useState<MemoryQueueSummary | null>(null);

  // Filter States
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCollection, setSelectedCollection] = useState<string>("all");
  const [selectedRelation, setSelectedRelation] = useState<string>("all");
  const [includeInactive, setIncludeInactive] = useState<boolean>(false);

  // Unresolved Conflicts Mode State
  const [conflictsMode, setConflictsMode] = useState<boolean>(false);
  const [conflicts, setConflicts] = useState<MemoryConflict[]>([]);

  // UI Panels
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [filterMenuOpen, setFilterMenuOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Measure container dimensions
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

  // Fetch graph topology ONLY when graph_version or includeInactive changes
  const fetchTopology = useCallback(
    async (force = false) => {
      try {
        const currentVersion = await getGraphVersion();
        if (force || currentVersion !== lastVersion) {
          setError(null);
          const payload = await getMemoryGraphTopology(includeInactive);
          if (payload) {
            setNodes(payload.nodes);
            setEdges(payload.edges);
            setLastVersion(payload.version);
          }
        }
      } catch (e: any) {
        console.error("Failed to fetch topology:", e);
        setError(e?.message || "Failed to load memory graph topology");
      }
    },
    [lastVersion, includeInactive]
  );

  // Fetch queue summary and conflicts status
  const fetchAuxiliaryData = useCallback(async () => {
    try {
      const [qData, cData] = await Promise.all([
        getMemoryQueueStatus(),
        getUnresolvedConflicts(),
      ]);
      setQueueSummary(qData);
      setConflicts(cData);
    } catch (e) {
      console.error("Auxiliary data load failed:", e);
    }
  }, []);

  // Initial load and version polling loop (every 2.5s)
  useEffect(() => {
    fetchTopology(true);
    fetchAuxiliaryData();

    const interval = setInterval(() => {
      fetchTopology(false);
      fetchAuxiliaryData();
    }, 2500);

    return () => clearInterval(interval);
  }, [fetchTopology, fetchAuxiliaryData]);

  // Re-fetch topology immediately when includeInactive toggle changes
  useEffect(() => {
    fetchTopology(true);
  }, [includeInactive, fetchTopology]);

  // Lazy load full fact detail when a node is selected
  const handleSelectNode = useCallback(
    async (nodeId: string | null, pos?: { x: number; y: number }) => {
      setSelectedNodeId(nodeId);
      if (!nodeId) {
        setSelectedFactDetail(null);
        setTooltipPos(null);
        return;
      }

      if (pos) {
        setTooltipPos(pos);
      }

      setIsDetailLoading(true);
      try {
        const detail = await getMemoryFactDetail(nodeId);
        setSelectedFactDetail(detail);
      } catch (e) {
        console.error("Lazy detail load failed:", e);
      } finally {
        setIsDetailLoading(false);
      }
    },
    []
  );

  // Resolve conflict handler
  const handleResolveConflict = useCallback(
    async (winnerId: string, loserId: string) => {
      try {
        await resolveMemoryConflict(winnerId, loserId);
        fetchAuxiliaryData();
        fetchTopology(true);
      } catch (e) {
        console.error("Resolve memory conflict failed:", e);
      }
    },
    [fetchAuxiliaryData, fetchTopology]
  );

  const totalPending = queueSummary
    ? (queueSummary.staged_pending ?? 0) +
      (queueSummary.dedup_pass ?? 0) +
      (queueSummary.nli_evaluated ?? 0) +
      (queueSummary.paused ?? 0)
    : 0;

  return (
    <div className="flex-1 relative overflow-hidden select-none w-full h-full bg-[rgb(var(--background))]">
      {/* Ambient background effect */}
      <AmbientBackground mood="calm" originX="50%" originY="50%" />

      {/* Main Graph Canvas */}
      <div ref={containerRef} className="absolute inset-0 z-10">
        {dims.w > 0 && (
          <MemoryGraph
            ref={graphRef}
            nodes={nodes}
            edges={edges}
            width={dims.w}
            height={dims.h}
            searchQuery={searchQuery}
            selectedCollection={selectedCollection}
            selectedRelation={selectedRelation}
            onSelectNode={handleSelectNode}
            selectedFactId={selectedNodeId}
            selectedFactDetail={selectedFactDetail}
            conflictPairs={conflictsMode ? conflicts : []}
          />
        )}
      </div>

      {/* Error Banner */}
      <AnimatePresence>
        {error && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="absolute top-16 left-1/2 -translate-x-1/2 z-30 pointer-events-auto max-w-md w-full px-4"
          >
            <div className="glass-card p-3 rounded-2xl border border-red-500/30 bg-[rgba(20,10,12,0.9)] backdrop-blur-xl flex items-center justify-between gap-3 shadow-2xl">
              <div className="flex items-center gap-2.5 overflow-hidden">
                <div className="w-2 h-2 rounded-full bg-red-500 animate-pulse shrink-0" />
                <p className="text-[11px] font-mono text-red-200 truncate">{error}</p>
              </div>
              <button
                onClick={() => fetchTopology(true)}
                className="px-2.5 py-1 rounded-lg text-[10px] font-mono font-bold uppercase tracking-wider bg-red-500/20 text-red-300 hover:bg-red-500/30 transition-colors shrink-0 cursor-pointer"
              >
                Retry
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Top-Center Search Bar & Filter Popover */}
      <div className="absolute top-4 left-1/2 -translate-x-1/2 z-20 pointer-events-auto flex items-center gap-2">
        <div className="glass-card h-[42px] px-3.5 rounded-full border border-[rgba(var(--accent),0.18)] bg-[rgba(10,12,14,0.70)] backdrop-blur-xl flex items-center gap-2.5 shadow-2xl w-[340px] focus-within:w-[420px] transition-all duration-300">
          <Search size={14} className="text-[rgb(var(--accent))] shrink-0" />
          <input
            type="text"
            placeholder="Search Fact ID, collection, session, or text..."
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

          {/* Filter Popover Toggle */}
          <div className="relative shrink-0">
            <button
              ref={filterBtnRef}
              onClick={() => setFilterMenuOpen((v) => !v)}
              className={cn(
                "p-1.5 rounded-full text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] transition-colors cursor-pointer",
                (filterMenuOpen || selectedCollection !== "all" || includeInactive || conflictsMode) &&
                  "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10"
              )}
              title="Filter by Collection or Status"
            >
              <SlidersHorizontal size={13} />
            </button>

            {/* Filter Menu Dropdown */}
            <AnimatePresence>
              {filterMenuOpen && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.95, y: 6 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.95, y: 6 }}
                  transition={{ duration: 0.15 }}
                  className="absolute right-0 top-10 w-[240px] glass-card p-3.5 rounded-2xl border border-[rgba(var(--accent),0.15)] bg-[rgba(10,12,14,0.92)] backdrop-blur-2xl shadow-2xl flex flex-col gap-2.5 z-30"
                >
                  <div className="flex items-center justify-between border-b border-white/[0.06] pb-1.5">
                    <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
                      Graph Filters
                    </span>
                    {(selectedCollection !== "all" || includeInactive || conflictsMode) && (
                      <button
                        onClick={() => {
                          setSelectedCollection("all");
                          setIncludeInactive(false);
                          setConflictsMode(false);
                        }}
                        className="text-[9px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
                      >
                        RESET
                      </button>
                    )}
                  </div>

                  {/* Toggle: Historical / Inactive Facts */}
                  <label className="flex items-center justify-between px-2 py-1.5 rounded-xl bg-white/[0.02] border border-white/[0.04] cursor-pointer">
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground))]">
                      Show Historical / Inactive Facts
                    </span>
                    <input
                      type="checkbox"
                      checked={includeInactive}
                      onChange={(e) => setIncludeInactive(e.target.checked)}
                      className="accent-[rgb(var(--accent))] cursor-pointer"
                    />
                  </label>

                  {/* Toggle: Unresolved Conflicts Mode */}
                  <label className="flex items-center justify-between px-2 py-1.5 rounded-xl bg-white/[0.02] border border-white/[0.04] cursor-pointer">
                    <div className="flex items-center gap-1.5">
                      <ShieldAlert size={12} className="text-red-400" />
                      <span className="text-[10px] font-mono text-[rgb(var(--foreground))]">
                        Unresolved Conflicts Mode ({conflicts.length})
                      </span>
                    </div>
                    <input
                      type="checkbox"
                      checked={conflictsMode}
                      onChange={(e) => setConflictsMode(e.target.checked)}
                      className="accent-red-500 cursor-pointer"
                    />
                  </label>

                  {/* Collection Picker */}
                  <div className="flex flex-col gap-1 border-t border-white/[0.06] pt-1.5">
                    <span className="text-[9px] font-mono font-bold text-[rgb(var(--foreground-muted))]/60 uppercase px-1">
                      Filter Collection
                    </span>
                    {["all", "Identity", "Profile", "Directives", "Constraints", "Entities", "Narrative", "Inactive"].map(
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

      {/* Top-Left: Collection Legend Card */}
      <div className="absolute top-4 left-4 z-20 pointer-events-auto">
        <MemoryLegendCard
          selectedCollection={selectedCollection}
          onSelectCollection={setSelectedCollection}
          selectedRelation={selectedRelation}
          onSelectRelation={setSelectedRelation}
          totalFactsCount={nodes.length}
          totalRelationsCount={edges.length}
        />
      </div>

      {/* Top-Right: Knowledge Base Metrics Card & Recenter Camera */}
      <div className="absolute top-4 right-4 z-20 pointer-events-auto flex flex-col items-end gap-2">
        <MemoryMetricsCard
          totalFacts={nodes.length}
          totalRelations={edges.length}
          nodes={nodes}
          edges={edges}
        />

        <button
          onClick={() => graphRef.current?.recenter()}
          className="flex items-center gap-1.5 px-3 py-2 rounded-full glass-card border border-[rgba(var(--accent),0.15)] bg-[rgba(10,12,14,0.70)] backdrop-blur-xl text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-all cursor-pointer shadow-xl text-[10px] font-mono font-bold uppercase tracking-wider"
          title="Recenter Graph View"
        >
          <Focus size={13} className="text-[rgb(var(--accent))]" />
          <span>Recenter Camera</span>
        </button>
      </div>

      {/* Unresolved Conflicts Floating Resolution Card */}
      <AnimatePresence>
        {conflictsMode && conflicts.length > 0 && (
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: -10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: -10 }}
            className="absolute top-20 left-1/2 -translate-x-1/2 z-20 pointer-events-auto w-[460px] max-w-[92vw]"
          >
            <div className="glass-card p-4 rounded-2xl border border-red-500/30 bg-[rgba(20,10,12,0.92)] backdrop-blur-2xl shadow-2xl flex flex-col gap-3">
              <div className="flex items-center justify-between border-b border-red-500/20 pb-2">
                <div className="flex items-center gap-2 text-red-400">
                  <AlertTriangle size={15} />
                  <span className="text-[11px] font-mono font-bold uppercase tracking-wider">
                    Unresolved Conflict Resolution ({conflicts.length})
                  </span>
                </div>
                <button
                  onClick={() => setConflictsMode(false)}
                  className="text-red-300 hover:text-white transition-colors cursor-pointer"
                >
                  <X size={14} />
                </button>
              </div>

              {/* Conflict Pair Resolution Item */}
              {conflicts.slice(0, 1).map((conflict) => (
                <div key={`${conflict.fact_a.id}_${conflict.fact_b.id}`} className="flex flex-col gap-2">
                  <div className="grid grid-cols-2 gap-2 text-[11px] font-mono">
                    <div className="p-2.5 rounded-xl bg-white/[0.03] border border-white/[0.06] flex flex-col justify-between gap-2">
                      <span className="font-bold text-[rgb(var(--accent))]">{conflict.fact_a.id}</span>
                      <span className="text-[10px] text-[rgb(var(--foreground-muted))] uppercase">
                        {conflict.fact_a.collection}
                      </span>
                      <button
                        onClick={() => handleResolveConflict(conflict.fact_a.id, conflict.fact_b.id)}
                        className="w-full py-1 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 text-[9px] font-bold uppercase hover:bg-emerald-500/30 transition-colors cursor-pointer"
                      >
                        Pick as Winner
                      </button>
                    </div>

                    <div className="p-2.5 rounded-xl bg-white/[0.03] border border-white/[0.06] flex flex-col justify-between gap-2">
                      <span className="font-bold text-red-400">{conflict.fact_b.id}</span>
                      <span className="text-[10px] text-[rgb(var(--foreground-muted))] uppercase">
                        {conflict.fact_b.collection}
                      </span>
                      <button
                        onClick={() => handleResolveConflict(conflict.fact_b.id, conflict.fact_a.id)}
                        className="w-full py-1 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 text-[9px] font-bold uppercase hover:bg-emerald-500/30 transition-colors cursor-pointer"
                      >
                        Pick as Winner
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Bottom Right Edge Nav: Ingestion Queue Trigger Button */}
      <div className="fixed bottom-4 right-4 z-30 pointer-events-auto">
        <button
          onClick={() => setDrawerOpen(true)}
          className={cn(
            "flex items-center gap-2.5 px-3.5 py-2.5 rounded-full glass-card border border-[rgba(var(--accent),0.25)] bg-[rgba(10,12,14,0.85)] hover:bg-[rgba(10,12,14,0.95)] backdrop-blur-xl text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.5)] transition-all cursor-pointer shadow-2xl group",
            drawerOpen && "opacity-0 pointer-events-none"
          )}
          title="Open Memory Ingestion Queue"
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
            Ingestion Queue {totalPending > 0 ? `(${totalPending})` : ""}
          </span>
        </button>
      </div>

      {/* Floating Memory Node Tooltip with Lazy Details */}
      <MemoryNodeTooltip
        factDetail={selectedFactDetail}
        isLoading={isDetailLoading}
        pos={tooltipPos}
        onClose={() => handleSelectNode(null)}
        onRefresh={() => {
          fetchTopology(true);
          fetchAuxiliaryData();
        }}
      />

      {/* Ingestion Queue Observability Drawer */}
      <MemoryPipelineDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        summary={queueSummary}
        nodes={nodes}
        edges={edges}
        onRefresh={() => {
          fetchTopology(true);
          fetchAuxiliaryData();
        }}
      />
    </div>
  );
};
