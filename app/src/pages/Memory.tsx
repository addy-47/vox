import React, { useState, useEffect, useRef, useCallback, useMemo } from "react";
import {
  Search,
  X,
  Focus,
  Eye,
  EyeOff,
  GitCompare,
  Cpu,
  CornerDownLeft,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { MemoryGraph, MemoryGraphRef, getCollectionColor } from "@/shared/components/memory/MemoryGraph";
import { MemoryLegendCard } from "@/shared/components/memory/MemoryLegendCard";
import { MemoryNodeTooltip } from "@/shared/components/memory/MemoryNodeTooltip";
import { MemoryPipelineDrawer } from "@/shared/components/memory/MemoryPipelineDrawer";
import {
  MemoryNodeTopology,
  MemoryEdgeTopology,
  MemoryFactDetail,
  MemoryQueueSummary,
  getMemoryGraphTopology,
  getMemoryFactDetail,
  getMemoryQueueStatus,
  getUnresolvedConflicts,
  getGraphVersion,
} from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

interface SearchBarProps {
  nodes: MemoryNodeTopology[];
  onCommitSearch: (query: string) => void;
  onSelectNode: (nodeId: string) => void;
}

const SearchBar: React.FC<SearchBarProps> = React.memo(({ nodes, onCommitSearch, onSelectNode }) => {
  const [input, setInput] = useState("");
  const [focused, setFocused] = useState(false);

  const searchResults = useMemo(() => {
    const q = input.trim().toLowerCase();
    if (!q) return [];
    return nodes
      .filter((n) => n.id.toLowerCase().includes(q) || n.collection.toLowerCase().includes(q))
      .slice(0, 10);
  }, [nodes, input]);

  return (
    <div className="absolute top-4 left-1/2 -translate-x-1/2 z-20 pointer-events-auto flex flex-col items-center">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          onCommitSearch(input.trim());
          setFocused(false);
        }}
        className="glass-card h-[42px] px-3.5 rounded-full border border-[rgba(var(--accent),0.2)] bg-[rgb(var(--card))]/90 backdrop-blur-2xl flex items-center gap-2.5 shadow-2xl w-[360px] focus-within:w-[440px] transition-all duration-300 relative"
      >
        <Search size={15} className="text-[rgb(var(--accent))] shrink-0" />
        <input
          type="text"
          placeholder="Search memory..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onFocus={() => setFocused(true)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              onCommitSearch(input.trim());
              setFocused(false);
            }
          }}
          className="w-full bg-transparent text-[12px] font-mono tracking-wide text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/50 outline-none"
        />

        <span className="text-[9px] font-mono font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--foreground))]/10 text-[rgb(var(--foreground-muted))] shrink-0">
          ⌘K
        </span>

        <AnimatePresence>
          {input && (
            <motion.button
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.8 }}
              type="button"
              onClick={() => {
                setInput("");
                onCommitSearch("");
              }}
              className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0"
            >
              <X size={13} />
            </motion.button>
          )}
        </AnimatePresence>
      </form>

      {/* Subpanel 4: Search Dropdown Results Popover */}
      {focused && searchResults.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 6 }}
          className="mt-2 w-[440px] glass-card p-2 rounded-2xl border border-[rgba(var(--accent),0.25)] bg-[rgb(var(--card))]/95 backdrop-blur-2xl shadow-2xl flex flex-col gap-1 z-30"
        >
          <div className="px-3 py-1.5 flex items-center justify-between border-b border-[rgba(var(--border),0.12)] text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
            <span>Results ({searchResults.length})</span>
            <span>Press Enter to filter graph</span>
          </div>

          <div className="flex flex-col gap-1 max-h-[220px] overflow-y-auto custom-scrollbar">
            {searchResults.map((node) => {
              const colPalette = getCollectionColor(node.collection, node.is_superseded);
              return (
                <button
                  key={node.id}
                  onClick={() => {
                    onSelectNode(node.id);
                    setFocused(false);
                  }}
                  className="p-2.5 rounded-xl bg-[rgb(var(--foreground))]/5 hover:bg-[rgb(var(--accent))]/15 border border-[rgba(var(--border),0.12)] flex items-center justify-between text-left transition-colors cursor-pointer group"
                >
                  <div className="flex items-center gap-2 overflow-hidden">
                    <span
                      className="text-[9px] font-mono px-1.5 py-0.5 rounded font-bold uppercase shrink-0"
                      style={{ backgroundColor: `${colPalette.main}20`, color: colPalette.main }}
                    >
                      {node.collection}
                    </span>
                    <span className="text-[11px] font-mono font-bold text-[rgb(var(--foreground))] truncate">
                      {node.id}
                    </span>
                  </div>
                  <CornerDownLeft size={12} className="text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))] shrink-0" />
                </button>
              );
            })}
          </div>
        </motion.div>
      )}
    </div>
  );
});

SearchBar.displayName = "SearchBar";

export const Memory: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<MemoryGraphRef>(null);

  const [dims, setDims] = useState<{ w: number; h: number }>({ w: 0, h: 0 });
  const [nodes, setNodes] = useState<MemoryNodeTopology[]>([]);
  const [edges, setEdges] = useState<MemoryEdgeTopology[]>([]);
  const [lastVersion, setLastVersion] = useState<number>(0);

  const [searchQuery, setSearchQuery] = useState("");

  const [selectedCollection, setSelectedCollection] = useState("all");
  const [selectedRelation, setSelectedRelation] = useState("all");
  const [includeInactive, setIncludeInactive] = useState(false);
  const [conflictsMode, setConflictsMode] = useState(false);

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [nodePos, setNodePos] = useState<{ x: number; y: number } | null>(null);
  const [selectedFactDetail, setSelectedFactDetail] = useState<MemoryFactDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

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

  // Fetch graph topology from backend IPC
  const fetchTopology = useCallback(
    async (force = false) => {
      try {
        const currentVer = await getGraphVersion();
        if (!force && currentVer === lastVersion && currentVer > 0) {
          return;
        }

        const data = await getMemoryGraphTopology(includeInactive);
        if (data) {
          setNodes(data.nodes);
          setEdges(data.edges);
          setLastVersion(currentVer);
        }
        setError(null);
      } catch (e: any) {
        console.error("Failed to load memory graph topology:", e);
        setError(e.message || "Failed to load knowledge graph from local database.");
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

  // Polling loop (every 2.5s) — Paused when drawer is open
  useEffect(() => {
    fetchTopology(true);
    fetchAuxiliaryData();

    const interval = setInterval(() => {
      if (!drawerOpen) {
        fetchTopology(false);
      }
      fetchAuxiliaryData();
    }, 2500);

    return () => clearInterval(interval);
  }, [fetchTopology, fetchAuxiliaryData, drawerOpen]);

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

  const totalPending = queueSummary
    ? (queueSummary.staged_pending || 0) +
      (queueSummary.dedup_pass || 0) +
      (queueSummary.nli_evaluated || 0)
    : 0;

  return (
    <div className="flex-1 relative overflow-hidden select-none w-full h-full bg-[rgb(var(--background))]">
      {/* Main WebGL GPU Graph Canvas */}
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
            <div className="glass-card p-3 rounded-2xl border border-red-500/30 bg-[rgb(var(--card))]/95 flex items-center justify-between gap-3 shadow-2xl">
              <div className="flex items-center gap-2.5 overflow-hidden">
                <div className="w-2 h-2 rounded-full bg-red-500 animate-pulse shrink-0" />
                <p className="text-[11px] font-mono text-red-400 truncate">{error}</p>
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

      {/* Subpanel 4: Search Experience Bar & Dropdown Popover */}
      <SearchBar nodes={nodes} onCommitSearch={setSearchQuery} onSelectNode={handleSelectNode} />

      {/* Top-Left: Card-Style Legend UI */}
      <div className="absolute top-4 left-4 z-20 pointer-events-auto">
        <MemoryLegendCard
          selectedCollection={selectedCollection}
          onSelectCollection={setSelectedCollection}
          selectedRelation={selectedRelation}
          onSelectRelation={setSelectedRelation}
        />
      </div>

      {/* Right Action Dock (Vertically Centered & Enlarged) */}
      <div className="absolute top-1/2 -translate-y-1/2 right-6 z-20 pointer-events-auto flex flex-col gap-3 p-2 rounded-2xl glass-card border border-[rgba(var(--accent),0.25)] bg-[rgb(var(--card))]/90 backdrop-blur-2xl shadow-2xl">
        {/* Button 1: 🎯 Recenter */}
        <button
          onClick={() => graphRef.current?.recenter()}
          className="w-10 h-10 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer relative group"
          title="Recenter Camera View"
        >
          <Focus size={18} />
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-mono whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            Recenter View
          </span>
        </button>

        {/* Button 2: 👁️ Historical / Inactive Facts Toggle */}
        <button
          onClick={() => setIncludeInactive((prev) => !prev)}
          className={cn(
            "w-10 h-10 flex items-center justify-center rounded-xl transition-all cursor-pointer relative group",
            includeInactive
              ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10"
          )}
          title="Toggle Historical / Inactive Facts"
        >
          {includeInactive ? <Eye size={18} /> : <EyeOff size={18} />}
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-mono whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            {includeInactive ? "Hide Inactive Facts" : "Show Inactive Facts"}
          </span>
        </button>

        {/* Button 3: ⚠️ Unresolved Conflicts Mode Toggle */}
        <button
          onClick={() => setConflictsMode((prev) => !prev)}
          className={cn(
            "w-10 h-10 flex items-center justify-center rounded-xl transition-all cursor-pointer relative group",
            conflictsMode
              ? "text-red-400 bg-red-500/20 border border-red-500/40"
              : "text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-[rgb(var(--foreground))]/10"
          )}
          title={`Unresolved Conflicts Mode (${conflicts.length})`}
        >
          <GitCompare size={18} />
          {conflicts.length > 0 && (
            <span className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-[9px] font-mono font-bold text-white shadow-md">
              {conflicts.length}
            </span>
          )}
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-mono whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            {`Unresolved Conflicts (${conflicts.length})`}
          </span>
        </button>

        {/* Button 4: ⚙️ Memory Ingestion Queue Trigger */}
        <button
          onClick={() => setDrawerOpen(true)}
          className={cn(
            "w-10 h-10 flex items-center justify-center rounded-xl transition-all cursor-pointer relative group",
            drawerOpen
              ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/20 border border-[rgb(var(--accent))]/40"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/10"
          )}
          title="Memory Ingestion Queue"
        >
          <Cpu size={18} />
          {totalPending > 0 && (
            <span className="absolute -top-1 -right-1 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-[rgb(var(--accent))] text-[9px] font-mono font-black text-black animate-pulse shadow-md" />
          )}
          <span className="absolute right-full top-1/2 -translate-y-1/2 mr-3 hidden group-hover:block px-3 py-1.5 rounded-xl bg-[rgb(var(--card))] text-[rgb(var(--foreground))] text-[11px] font-mono whitespace-nowrap z-30 shadow-2xl border border-[rgba(var(--border),0.2)]">
            Memory Ingestion Queue
          </span>
        </button>
      </div>

      {/* Subpanel 3: Node Detail Tooltip Popover */}
      {selectedNodeId && (
        <MemoryNodeTooltip
          factDetail={selectedFactDetail}
          isLoading={detailLoading}
          pos={nodePos}
          onClose={() => handleSelectNode(null)}
          onRefresh={() => fetchTopology(true)}
        />
      )}

      {/* Subpanel 2: Ingestion Queue Slide-in Drawer */}
      <MemoryPipelineDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        summary={queueSummary}
        nodes={nodes}
        onRefresh={fetchAuxiliaryData}
      />
    </div>
  );
};
