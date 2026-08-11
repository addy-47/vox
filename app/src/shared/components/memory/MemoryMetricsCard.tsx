import React, { useState, useMemo } from "react";
import { ChevronDown, ChevronUp, Database, ArrowRight } from "lucide-react";
import { MemoryNodeTopology, MemoryEdgeTopology } from "@/services/memoryService";
import { COLLECTION_COLORS } from "@/shared/components/memory/MemoryGraph";
import { cn } from "@/shared/lib/utils";

interface MemoryMetricsCardProps {
  totalFacts: number;
  totalRelations: number;
  nodes: MemoryNodeTopology[];
  edges: MemoryEdgeTopology[];
}

interface CollectionPairMetrics {
  pairKey: string;
  fromCol: string;
  toCol: string;
  totalEdges: number;
  relationCounts: Record<string, number>;
}

export const MemoryMetricsCard: React.FC<MemoryMetricsCardProps> = ({
  totalFacts,
  totalRelations,
  nodes,
  edges,
}) => {
  const [expanded, setExpanded] = useState(false);
  const [openPair, setOpenPair] = useState<string | null>(null);

  // Group directed edges by distinct collection pair (e.g. "Identity → Profile")
  const pairMetrics = useMemo(() => {
    const nodeMap = new Map<string, string>();
    nodes.forEach((n) => nodeMap.set(n.id, n.collection || "Identity"));

    const map = new Map<string, CollectionPairMetrics>();

    edges.forEach((edge) => {
      const fromCol = nodeMap.get(edge.from_id) || "Identity";
      const toCol = nodeMap.get(edge.to_id) || "Identity";
      const pairKey = `${fromCol} → ${toCol}`;

      let entry = map.get(pairKey);
      if (!entry) {
        entry = {
          pairKey,
          fromCol,
          toCol,
          totalEdges: 0,
          relationCounts: {},
        };
        map.set(pairKey, entry);
      }

      entry.totalEdges += 1;
      const relNorm = edge.relation ? edge.relation.toUpperCase() : "OTHER";
      entry.relationCounts[relNorm] = (entry.relationCounts[relNorm] || 0) + 1;
    });

    return Array.from(map.values()).sort((a, b) => b.totalEdges - a.totalEdges);
  }, [nodes, edges]);

  return (
    <div className="glass-card rounded-2xl border border-[rgba(var(--accent),0.15)] bg-[rgba(10,12,14,0.85)] backdrop-blur-2xl shadow-2xl pointer-events-auto transition-all duration-200 w-[280px]">
      {/* Collapsed Mode / Header */}
      <div
        onClick={() => setExpanded((prev) => !prev)}
        className="px-3.5 py-2.5 flex items-center justify-between cursor-pointer hover:bg-white/[0.03] rounded-2xl transition-colors select-none"
      >
        <div className="flex items-center gap-2">
          <Database size={14} className="text-[rgb(var(--accent))]" />
          <div className="flex flex-col">
            <span className="text-[10px] font-mono font-bold tracking-[0.12em] uppercase text-[rgb(var(--foreground))]/90">
              KNOWLEDGE BASE METRICS
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/80 font-bold">
              {totalFacts.toLocaleString()} Facts · {totalRelations} Relations
            </span>
          </div>
        </div>
        <button
          className="p-1 rounded-lg text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors"
          title={expanded ? "Collapse Metrics" : "Expand Metrics"}
        >
          {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
        </button>
      </div>

      {/* Expanded Accordion Breakdown */}
      {expanded && (
        <div className="px-3.5 pb-3 pt-1 border-t border-white/[0.06] flex flex-col gap-2 max-h-[320px] overflow-y-auto custom-scrollbar">
          <div className="flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 pb-1">
            <span>COLLECTION PAIRS</span>
            <span>EDGES</span>
          </div>

          {pairMetrics.length > 0 ? (
            pairMetrics.map((pair) => {
              const isOpen = openPair === pair.pairKey;
              const fromStyle = (COLLECTION_COLORS as any)[pair.fromCol] || COLLECTION_COLORS.Identity;
              const toStyle = (COLLECTION_COLORS as any)[pair.toCol] || COLLECTION_COLORS.Identity;

              return (
                <div
                  key={pair.pairKey}
                  className="rounded-xl border border-white/[0.05] bg-white/[0.02] overflow-hidden"
                >
                  {/* Pair Header Button */}
                  <button
                    onClick={() => setOpenPair(isOpen ? null : pair.pairKey)}
                    className="w-full px-2.5 py-1.5 flex items-center justify-between text-left hover:bg-white/[0.04] transition-colors cursor-pointer"
                  >
                    <div className="flex items-center gap-1.5 text-[10px] font-mono font-semibold">
                      <span className="truncate max-w-[80px]" style={{ color: fromStyle.main }}>
                        {pair.fromCol}
                      </span>
                      <ArrowRight size={10} className="text-[rgb(var(--foreground-muted))]/50 shrink-0" />
                      <span className="truncate max-w-[80px]" style={{ color: toStyle.main }}>
                        {pair.toCol}
                      </span>
                    </div>

                    <div className="flex items-center gap-1.5">
                      <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
                        {pair.totalEdges}
                      </span>
                      <ChevronDown
                        size={12}
                        className={cn(
                          "text-[rgb(var(--foreground-muted))]/60 transition-transform duration-200",
                          isOpen && "rotate-180"
                        )}
                      />
                    </div>
                  </button>

                  {/* Accordion Relations Breakdown */}
                  {isOpen && (
                    <div className="px-2.5 py-2 border-t border-white/[0.04] bg-black/20 grid grid-cols-2 gap-1.5 text-[9px] font-mono">
                      {["SUPPORTS", "DEPENDS_ON", "SHAPES", "CONFLICTS_WITH", "SUPERSEDES", "restricted_by"].map((rel) => {
                        const count = pair.relationCounts[rel] || pair.relationCounts[rel.toLowerCase()] || 0;
                        if (count === 0) return null;
                        return (
                          <div
                            key={rel}
                            className="flex items-center justify-between px-1.5 py-0.5 rounded bg-white/[0.03] border border-white/[0.04]"
                          >
                            <span className="text-[rgb(var(--foreground-muted))]/80 truncate">
                              {rel}
                            </span>
                            <span className="font-bold text-[rgb(var(--foreground))] ml-1">
                              {count}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          ) : (
            <div className="py-3 text-center text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 italic">
              No collection relations recorded
            </div>
          )}
        </div>
      )}
    </div>
  );
};
