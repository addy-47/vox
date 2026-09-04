import React, { useState, useEffect, useCallback, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Focus,
  Eye,
  EyeOff,
  GitCompare,
  Cpu,
  RefreshCw,
  Plus,
  Minus,
  MousePointerClick,
  ChevronLeft,
  ChevronRight,
  Search,
} from "lucide-react";
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
import { MEMORY_COPY } from "@/data/memoryCopy";
import { HelpTriggerButton } from "@/shared/components/help/HelpTriggerButton";
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

  const [selectMode, setSelectMode] = useState(false);
  const [includeInactive, setIncludeInactive] = useState(false);
  const [conflictsMode, setConflictsMode] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [mobileDockExpanded, setMobileDockExpanded] = useState(false);
  const [isMobileSearchOpen, setIsMobileSearchOpen] = useState(false);

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

  const toastTimerRef = useRef<NodeJS.Timeout | null>(null);
  useEffect(() => {
    return () => {
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    };
  }, []);

  // Explicit user-triggered Graph Refresh with version check
  const handleRefreshGraph = useCallback(async () => {
    setIsRefreshing(true);
    setUpToDateToast(false);
    try {
      const currentVer = await getGraphVersion();
      if (currentVer > lastVersionRef.current) {
        await fetchTopology(true);
      } else {
        if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
        setUpToDateToast(true);
        toastTimerRef.current = setTimeout(() => setUpToDateToast(false), 2500);
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

  // Fetch auxiliary queue stats on mount
  useEffect(() => {
    fetchAuxiliaryData();
  }, [fetchAuxiliaryData]);

  // Fetch topology on mount and when includeInactive toggle changes
  useEffect(() => {
    fetchTopology(true);
  }, [includeInactive, fetchTopology]);

  const selectedNodeIdRef = useRef<string | null>(null);
  selectedNodeIdRef.current = selectedNodeId;

  // Lazy load full fact detail when a node is selected (toggles off if clicking selected node again)
  const handleSelectNode = useCallback(
    async (nodeId: string | null, pos?: { x: number; y: number }) => {
      const isMobile = typeof window !== "undefined" ? window.innerWidth < 640 : false;
      // On small layout, direct node tap only opens tooltip when selectMode is active (or clearing selection)
      if (isMobile && nodeId && !selectMode) {
        return;
      }

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
    [selectMode]
  );

  const handleCloseTooltip = useCallback(() => {
    handleSelectNode(null);
  }, [handleSelectNode]);

  const handleRefreshTopology = useCallback(() => {
    fetchTopology(true);
  }, [fetchTopology]);

  const handleCloseDrawer = useCallback(() => {
    setDrawerOpen(false);
  }, []);

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

      {/* ── Desktop Legend Card (bottom-right corner aligned with EdgeNav, hidden on < 640px) ── */}
      <div className="absolute bottom-4 right-6 z-30 pointer-events-auto hidden sm:block">
        <MemoryLegendCard
          selectedCollection={selectedCollection}
          onSelectCollection={setSelectedCollection}
          selectedRelation={selectedRelation}
          onSelectRelation={setSelectedRelation}
        />
      </div>

      {/* ── Desktop Search Bar (sm: >= 640px) ─────────────────────────────── */}
      <div className="hidden sm:block">
        <SearchBar
          variant="full"
          nodes={nodes}
          onCommitSearch={setSearchQuery}
          onSelectNode={handleSelectNode}
        />
      </div>

      {/* ── Desktop Zoom Controls Dock (sm: >= 640px) ─────────────────────── */}
      <div className="absolute top-4 right-6 z-20 pointer-events-auto hidden sm:flex items-center gap-1.5 p-1.5 rounded-2xl glass-card border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-2xl shadow-2xl">
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

      {/* ── Small / Mobile Layout Header & Horizontal Action Bar (< 640px) ── */}
      <AnimatePresence mode="wait">
        {isMobileSearchOpen ? (
          /* Mobile Search Overlay: Smoothly Expands Across Top Header */
          <motion.div
            key="mobile-search-overlay"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 20 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="absolute top-4 left-4 right-4 z-40 pointer-events-auto sm:hidden"
          >
            <SearchBar
              variant="full"
              className="w-full"
              nodes={nodes}
              onCommitSearch={setSearchQuery}
              onSelectNode={handleSelectNode}
              onClose={() => {
                setIsMobileSearchOpen(false);
                setSearchQuery("");
              }}
              autoFocus
            />
          </motion.div>
        ) : (
          /* Mobile Header with Horizontal Dynamic Action Tray */
          <motion.div
            key="mobile-header-bar"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute top-4 left-4 right-4 z-20 pointer-events-none sm:hidden flex items-center justify-between gap-2"
          >
            {/* Top-Left Title */}
            <div className="flex flex-col pointer-events-auto shrink-0">
              <h1 className="text-[14px] font-display font-black uppercase tracking-[0.18em] text-[rgb(var(--foreground))]">
                {MEMORY_COPY.memoryTitle}
              </h1>
              <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
                {MEMORY_COPY.memorySubtitle}
              </span>
            </div>

            {/* Top-Right Horizontal Dynamic Action Tray */}
            <div className="pointer-events-auto flex items-center gap-1 p-1 rounded-2xl glass-card border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-2xl shadow-2xl overflow-hidden max-w-[calc(100vw-150px)]">
              <HelpTriggerButton deepLink="page:memory" size="sm" />
              {/* Search Trigger */}
              <button
                onClick={() => setIsMobileSearchOpen(true)}
                aria-label={MEMORY_COPY.searchMemories}
                className="w-8 h-8 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer shrink-0"
              >
                <Search size={15} />
              </button>

              {/* Recenter */}
              <button
                onClick={() => graphRef.current?.recenter()}
                aria-label={MEMORY_COPY.recenterView}
                className="w-8 h-8 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer shrink-0"
              >
                <Focus size={15} />
              </button>

              {/* Select Mode */}
              <button
                onClick={() => {
                  setSelectMode((prev) => {
                    if (prev) {
                      setSelectedNodeId(null);
                      setSelectedFactDetail(null);
                      setNodePos(null);
                    }
                    return !prev;
                  });
                }}
                className={cn(
                  "w-8 h-8 flex items-center justify-center rounded-xl transition-all cursor-pointer shrink-0",
                  selectMode
                    ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/25 border border-[rgb(var(--accent))]/50 shadow-[0_0_12px_rgba(var(--accent),0.35)]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10"
                )}
                aria-label={selectMode ? MEMORY_COPY.disableSelectMode : MEMORY_COPY.enableSelectMode}
              >
                <MousePointerClick size={15} />
              </button>

              {/* Dynamic Horizontal Expander */}
              <AnimatePresence initial={false}>
                {mobileDockExpanded && (
                  <motion.div
                    initial={{ width: 0, opacity: 0 }}
                    animate={{ width: "auto", opacity: 1 }}
                    exit={{ width: 0, opacity: 0 }}
                    transition={{ duration: 0.18, ease: "easeOut" }}
                    className="flex items-center gap-1 overflow-hidden"
                  >
                    <div className="w-[1px] h-4 bg-[rgba(var(--border),0.2)] shrink-0" />

                    <button
                      onClick={handleRefreshGraph}
                      disabled={isRefreshing}
                      className="w-8 h-8 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer disabled:opacity-40 shrink-0"
                      aria-label="Refresh Memories"
                    >
                      <RefreshCw size={15} className={cn(isRefreshing && "animate-spin")} />
                    </button>

                    <button
                      onClick={() => setIncludeInactive((prev) => !prev)}
                      className={cn(
                        "w-8 h-8 flex items-center justify-center rounded-xl transition-all cursor-pointer shrink-0",
                        includeInactive
                          ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40"
                          : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10"
                      )}
                      aria-label={includeInactive ? MEMORY_COPY.hideInactive : MEMORY_COPY.showInactive}
                    >
                      {includeInactive ? <Eye size={15} /> : <EyeOff size={15} />}
                    </button>

                    <button
                      onClick={() => setConflictsMode((prev) => !prev)}
                      className={cn(
                        "w-8 h-8 flex items-center justify-center rounded-xl transition-all cursor-pointer relative shrink-0",
                        conflictsMode
                          ? "text-red-400 bg-red-500/20 border border-red-500/40"
                          : "text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-[rgb(var(--foreground))]/10"
                      )}
                      aria-label={`${MEMORY_COPY.unresolvedConflicts} (${conflicts.length})`}
                    >
                      <GitCompare size={15} />
                      {conflicts.length > 0 && (
                        <span className="absolute -top-0.5 -right-0.5 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-red-500 text-[9px] font-mono font-bold text-white shadow-xs">
                          {conflicts.length}
                        </span>
                      )}
                    </button>

                    <button
                      onClick={() => setDrawerOpen((v) => !v)}
                      className="w-8 h-8 flex items-center justify-center rounded-xl transition-all cursor-pointer text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40 hover:bg-[rgb(var(--accent))]/30 shadow-xs shrink-0"
                      aria-label={MEMORY_COPY.ingestionQueue}
                    >
                      <Cpu size={15} />
                    </button>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Chevron Button (Expands/Collapses horizontally) */}
              <button
                onClick={() => setMobileDockExpanded((v) => !v)}
                aria-label={mobileDockExpanded ? "Collapse controls" : "Expand controls"}
                className={cn(
                  "w-8 h-8 flex items-center justify-center rounded-xl transition-all cursor-pointer text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 shrink-0",
                  mobileDockExpanded && "bg-[rgb(var(--accent))]/15"
                )}
              >
                {mobileDockExpanded ? <ChevronLeft size={15} /> : <ChevronRight size={15} />}
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Desktop Right Action Dock (sm: >= 640px) ─────────────────────────── */}
      <div className="absolute top-1/2 -translate-y-1/2 right-6 z-20 pointer-events-auto hidden sm:flex flex-col gap-3 p-2 rounded-2xl glass-card border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-2xl shadow-2xl">
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
          onClick={() => setDrawerOpen((v) => !v)}
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
          onClose={handleCloseTooltip}
          onRefresh={handleRefreshTopology}
        />
      )}

      {/* Ingestion Queue Slide-in Drawer */}
      <MemoryPipelineDrawer
        open={drawerOpen}
        onClose={handleCloseDrawer}
        summary={queueSummary}
        nodes={nodes}
        edges={edges}
        onRefresh={fetchAuxiliaryData}
      />
    </div>
  );
};
