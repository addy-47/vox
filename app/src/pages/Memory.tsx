import React, { useState, useEffect, useCallback, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Focus, Eye, EyeOff, GitCompare, Cpu, RefreshCw, Plus, Minus } from "lucide-react";
import { MemoryGraph, MemoryGraphRef } from "@/shared/components/memory/MemoryGraph";
import { SearchBar } from "@/shared/components/memory/SearchBar";
import { MemoryLegendCard } from "@/shared/components/memory/MemoryLegendCard";
import { MemoryNodeTooltip } from "@/shared/components/memory/MemoryNodeTooltip";
import { MemoryPipelineDrawer } from "@/shared/components/memory/MemoryPipelineDrawer";
import { AmbientBackground, ErrorBoundary } from "@/shared/components/common";
import {
  MemoryNodeTopology,
  MemoryEdgeTopology,
  MemoryQueueSummary,
  MemoryFactDetail,
  getMemoryGraphTopology,
  getGraphVersion,
  getMemoryQueueStatus,
  getUnresolvedConflicts,
  getMemoryFactDetail,
} from "@/services/memoryService";
import { MEMORY_COPY } from "@/data/memoryData";
import { cn } from "@/shared/lib/utils";

const EMPTY_CONFLICTS: { fact_a: MemoryNodeTopology; fact_b: MemoryNodeTopology }[] = [];

export const Memory: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<MemoryGraphRef>(null);

  const [dims, setDims] = useState<{ w: number; h: number }>({ w: 0, h: 0 });
  const [nodes, setNodes] = useState<MemoryNodeTopology[]>([]);
  const [edges, setEdges] = useState<MemoryEdgeTopology[]>([]);


  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCollection, setSelectedCollection] = useState("all");
  const [selectedRelation, setSelectedRelation] = useState("all");

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedFactDetail, setSelectedFactDetail] = useState<MemoryFactDetail | null>(null);
  const [nodePos, setNodePos] = useState<{ x: number; y: number } | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const [includeInactive, setIncludeInactive] = useState(false);
  const [conflictsMode, setConflictsMode] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const [queueSummary, setQueueSummary] = useState<MemoryQueueSummary | null>(null);
  const [conflicts, setConflicts] = useState<{ fact_a: MemoryNodeTopology; fact_b: MemoryNodeTopology }[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Measure container dimensions
  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setDims({
          w: entry.contentRect.width,
          h: entry.contentRect.height,
        });
      }
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, []);

  const lastVersionRef = useRef<number>(0);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [upToDateToast, setUpToDateToast] = useState(false);

  // Fetch graph topology from backend IPC
  const fetchTopology = useCallback(
    async (force = false) => {
      try {
        const currentVer = await getGraphVersion();
        if (!force && currentVer === lastVersionRef.current && currentVer > 0) {
          return false;
        }

        const data = await getMemoryGraphTopology(includeInactive);
        if (data) {
          setNodes(data.nodes);
          setEdges(data.edges);
          lastVersionRef.current = currentVer;
        }
        setError(null);
        return true;
      } catch (e: any) {
        console.error("Failed to load memory graph topology:", e);
        setError(e.message || "Couldn't load your memories.");
        return false;
      }
    },
    [includeInactive]
  );

  // Explicit user-triggered Graph Refresh with version check
  const handleRefreshGraph = useCallback(async () => {
    setIsRefreshing(true);
    setUpToDateToast(false);
    try {
      const currentVer = await getGraphVersion();
      if (currentVer > lastVersionRef.current) {
        await fetchTopology(true);
      } else {
        setUpToDateToast(true);
        setTimeout(() => setUpToDateToast(false), 2500);
      }
    } finally {
      setIsRefreshing(false);
    }
  }, [fetchTopology]);

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

  // Fetch topology ONCE on mount. Polling is disabled for graph topology; only auxiliary queue stats poll when drawer is open.
  useEffect(() => {
    fetchTopology(true);
    fetchAuxiliaryData();
  }, [fetchTopology, fetchAuxiliaryData]);

  // Re-fetch topology when includeInactive toggle changes
  useEffect(() => {
    fetchTopology(true);
  }, [includeInactive, fetchTopology]);

  const selectedNodeIdRef = useRef<string | null>(null);
  selectedNodeIdRef.current = selectedNodeId;

  // Lazy load full fact detail when a node is selected (toggles off if clicking selected node again)
  const handleSelectNode = useCallback(
    async (nodeId: string | null, pos?: { x: number; y: number }) => {
      if (nodeId && nodeId === selectedNodeIdRef.current) {
        setSelectedNodeId(null);
        setSelectedFactDetail(null);
        setNodePos(null);
        return;
      }

      setSelectedNodeId(nodeId);
      if (!nodeId) {
        setSelectedFactDetail(null);
        setNodePos(null);
        return;
      }

      if (pos) {
        setNodePos(pos);
      } else {
        setNodePos({ x: window.innerWidth / 2 - 160, y: window.innerHeight / 2 - 180 });
      }

      setDetailLoading(true);
      try {
        const detail = await getMemoryFactDetail(nodeId);
        setSelectedFactDetail(detail);
      } catch (e) {
        console.error("Failed to load memory fact detail:", e);
        setSelectedFactDetail(null);
      } finally {
        setDetailLoading(false);
      }
    },
    []
  );

  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Reactive Ambient Background — rendered directly behind 3D WebGL Graph canvas */}
      <AmbientBackground originX="50%" originY="50%" rippleSpeedMultiplier={1.5} />

      {/* Main Full-Bleed WebGL GPU Graph Canvas directly on ambient background */}
      <div ref={containerRef} className="absolute inset-0 z-0 bg-transparent">
        {dims.w > 0 && (
          <ErrorBoundary name="MemoryGraphCanvas">
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
              conflictPairs={conflictsMode ? conflicts : EMPTY_CONFLICTS}
            />
          </ErrorBoundary>
        )}
      </div>

      {/* Error & Up To Date Banners */}
      <AnimatePresence>
        {upToDateToast && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="absolute top-16 left-1/2 -translate-x-1/2 z-30 pointer-events-none max-w-xs w-full px-4"
          >
            <div className="glass-card px-4 py-2 rounded-2xl border border-[rgba(var(--accent),0.3)] bg-[rgb(var(--card))]/90 backdrop-blur-2xl flex items-center justify-center gap-2 shadow-2xl">
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
              <span className="text-[11px] font-sans font-bold uppercase tracking-wider text-[rgb(var(--accent))]">
                Memory is up to date
              </span>
            </div>
          </motion.div>
        )}

        {error && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="absolute top-16 left-1/2 -translate-x-1/2 z-30 pointer-events-auto max-w-md w-full px-4"
          >
            <div className="glass-card p-3.5 rounded-2xl border border-[rgb(var(--danger))]/30 bg-[rgb(var(--card))]/95 flex items-center justify-between gap-3 shadow-2xl">
              <div className="flex items-center gap-2.5 overflow-hidden">
                <div className="w-2.5 h-2.5 rounded-full bg-[rgb(var(--danger))] shrink-0" />
                <p className="text-[11px] font-sans text-[rgb(var(--danger))] truncate">{error}</p>
              </div>
              <button
                onClick={() => fetchTopology(true)}
                className="px-3 py-1 rounded-xl text-[11px] font-sans font-bold uppercase tracking-wider bg-[rgba(var(--danger),0.18)] text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.28)] transition-colors shrink-0 cursor-pointer shadow-sm"
              >
                Retry
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Search Bar & Popover */}
      <SearchBar nodes={nodes} onCommitSearch={setSearchQuery} onSelectNode={handleSelectNode} />

      {/* Top-Left: Legend Card (hidden on small layouts) */}
      <div className="absolute top-4 left-4 z-20 pointer-events-auto hidden sm:block">
        <MemoryLegendCard
          selectedCollection={selectedCollection}
          onSelectCollection={setSelectedCollection}
          selectedRelation={selectedRelation}
          onSelectRelation={setSelectedRelation}
        />
      </div>

      {/* Top-Right: Zoom Controls Dock */}
      <div className="absolute top-4 right-6 z-20 pointer-events-auto flex items-center gap-1.5 p-1.5 rounded-2xl glass-card border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-2xl shadow-2xl">
        <button
          onClick={() => graphRef.current?.zoomIn()}
          aria-label={MEMORY_COPY.zoomIn}
          className="w-8 h-8 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer relative group"
        >
          <Plus size={16} />
          <span className="absolute top-full mt-2 left-1/2 -translate-x-1/2 hidden group-hover:block px-2.5 py-1 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)] pointer-events-none">
            {MEMORY_COPY.zoomIn}
          </span>
        </button>

        <div className="w-[1px] h-4 bg-[rgba(var(--border),0.2)]" />

        <button
          onClick={() => graphRef.current?.zoomOut()}
          aria-label={MEMORY_COPY.zoomOut}
          className="w-8 h-8 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer relative group"
        >
          <Minus size={16} />
          <span className="absolute top-full mt-2 left-1/2 -translate-x-1/2 hidden group-hover:block px-2.5 py-1 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)] pointer-events-none">
            {MEMORY_COPY.zoomOut}
          </span>
        </button>
      </div>

      {/* Right Action Dock */}
      <div className="absolute top-1/2 -translate-y-1/2 right-6 z-20 pointer-events-auto flex flex-col gap-3 p-2 rounded-2xl glass-card border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-2xl shadow-2xl">
        <button
          onClick={handleRefreshGraph}
          disabled={isRefreshing}
          className="w-10 h-10 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer relative group disabled:opacity-40"
        >
          <RefreshCw size={18} className={cn(isRefreshing && "animate-spin")} />
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            Refresh Memories
          </span>
        </button>

        <button
          onClick={() => graphRef.current?.recenter()}
          className="w-10 h-10 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer relative group"
        >
          <Focus size={18} />
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            {MEMORY_COPY.recenterView}
          </span>
        </button>

        <button
          onClick={() => setIncludeInactive((prev) => !prev)}
          className={cn(
            "w-10 h-10 flex items-center justify-center rounded-xl transition-all cursor-pointer relative group",
            includeInactive
              ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10"
          )}
        >
          {includeInactive ? <Eye size={18} /> : <EyeOff size={18} />}
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            {includeInactive ? MEMORY_COPY.hideInactive : MEMORY_COPY.showInactive}
          </span>
        </button>

        <button
          onClick={() => setConflictsMode((prev) => !prev)}
          className={cn(
            "w-10 h-10 flex items-center justify-center rounded-xl transition-all cursor-pointer relative group",
            conflictsMode
              ? "text-red-400 bg-red-500/20 border border-red-500/40"
              : "text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-[rgb(var(--foreground))]/10"
          )}
        >
          <GitCompare size={18} />
          {conflicts.length > 0 && (
            <span className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-[11px] font-mono font-bold text-white shadow-md">
              {conflicts.length}
            </span>
          )}
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            {`${MEMORY_COPY.unresolvedConflicts} (${conflicts.length})`}
          </span>
        </button>

        <button
          onClick={() => setDrawerOpen(true)}
          className="w-10 h-10 flex items-center justify-center rounded-xl transition-all cursor-pointer relative group text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40 hover:bg-[rgb(var(--accent))]/30 shadow-md"
        >
          <Cpu size={18} />
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-sans whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            {MEMORY_COPY.ingestionQueue}
          </span>
        </button>
      </div>

      {/* Node Detail Tooltip Popover */}
      {selectedNodeId && (
        <MemoryNodeTooltip
          factDetail={selectedFactDetail}
          isLoading={detailLoading}
          pos={nodePos}
          onClose={() => handleSelectNode(null)}
          onRefresh={() => fetchTopology(true)}
        />
      )}

      {/* Ingestion Queue Slide-in Drawer */}
      <MemoryPipelineDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        summary={queueSummary}
        nodes={nodes}
        edges={edges}
        onRefresh={fetchAuxiliaryData}
      />
    </div>
  );
};
