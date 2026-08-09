import React, { useCallback, useEffect, useMemo, useRef } from "react";
import ForceGraph2D, { ForceGraphMethods, NodeObject, LinkObject } from "react-force-graph-2d";
import * as d3Force from "d3-force";
import { Crosshair } from "lucide-react";
import { MemoryFactEntry, MemoryRelationEntry } from "@/services/memoryService";

export interface GNode {
  id: string;
  label: string;
  collection: string;
  status: "active" | "inactive";
  factEntry: MemoryFactEntry;
  color: string;
  x?: number;
  y?: number;
  vx?: number;
  vy?: number;
}

export interface GLink {
  id: string;
  source: string | GNode;
  target: string | GNode;
  relation: string;
  color: string;
  isDashed: boolean;
}

// ─── Collection & Relation Color System ───────────────────────────────────

export const COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string }> = {
  Identity: { main: "#00f2fe", glow: "rgba(0, 242, 254, 0.4)", text: "#00f2fe" },
  Directives: { main: "#a855f7", glow: "rgba(168, 85, 247, 0.4)", text: "#a855f7" },
  Narrative: { main: "#3b82f6", glow: "rgba(59, 130, 246, 0.4)", text: "#3b82f6" },
  Profile: { main: "#10b981", glow: "rgba(16, 185, 129, 0.4)", text: "#10b981" },
  Entities: { main: "#38bdf8", glow: "rgba(56, 189, 248, 0.4)", text: "#38bdf8" },
  Constraints: { main: "#f59e0b", glow: "rgba(245, 158, 11, 0.4)", text: "#f59e0b" },
  Inactive: { main: "#64748b", glow: "rgba(100, 116, 139, 0.3)", text: "#64748b" },
};

export const RELATION_STYLES: Record<string, { color: string; isDashed: boolean }> = {
  SUPPORTS: { color: "#10b981", isDashed: false },
  SUPERSEDES: { color: "#00f2fe", isDashed: false },
  SHAPES: { color: "#3b82f6", isDashed: false },
  DEPENDS_ON: { color: "#f59e0b", isDashed: false },
  CONFLICTS_WITH: { color: "#ef4444", isDashed: true },
  restricted_by: { color: "#ef4444", isDashed: true },
  OTHER: { color: "#94a3b8", isDashed: true },
};

export function getCollectionColor(rawCollection: string, isSuperseded = false) {
  if (isSuperseded) return COLLECTION_COLORS.Inactive;
  const norm = rawCollection.toLowerCase();
  if (norm.includes("identity")) return COLLECTION_COLORS.Identity;
  if (norm.includes("directive")) return COLLECTION_COLORS.Directives;
  if (norm.includes("narrative") || norm.includes("context")) return COLLECTION_COLORS.Narrative;
  if (norm.includes("profile")) return COLLECTION_COLORS.Profile;
  if (norm.includes("entity") || norm.includes("project")) return COLLECTION_COLORS.Entities;
  if (norm.includes("constraint")) return COLLECTION_COLORS.Constraints;
  return COLLECTION_COLORS.Identity;
}

export function getRelationStyle(rawRelation: string) {
  const norm = rawRelation.toUpperCase();
  if (norm.includes("SUPPORT")) return RELATION_STYLES.SUPPORTS;
  if (norm.includes("SUPERSEDE")) return RELATION_STYLES.SUPERSEDES;
  if (norm.includes("SHAPE")) return RELATION_STYLES.SHAPES;
  if (norm.includes("DEPEND")) return RELATION_STYLES.DEPENDS_ON;
  if (norm.includes("CONFLICT") || norm.includes("RESTRICT")) return RELATION_STYLES.CONFLICTS_WITH;
  return RELATION_STYLES.OTHER;
}

// ─── Graph Data Builder (REAL NODES & REAL EDGES ONLY) ───────────────────

function buildRealDistributedGraph(
  facts: MemoryFactEntry[],
  relations: MemoryRelationEntry[],
  _width: number,
  _height: number
): { nodes: GNode[]; links: GLink[] } {
  // Graph origin (0,0) is rendered at canvas center by react-force-graph-2d
  const cx = 0;
  const cy = 0;

  const nodeMap = new Map<string, GNode>();

  facts.forEach((f, idx) => {
    const colPalette = getCollectionColor(f.collection, f.is_superseded);
    const angle = (idx / Math.max(1, facts.length)) * Math.PI * 2;
    const dist = 60 + Math.random() * 260;

    const node: GNode = {
      id: f.id,
      label: f.fact,
      collection: f.collection,
      status: f.is_superseded ? "inactive" : "active",
      factEntry: f,
      color: colPalette.main,
      x: cx + Math.cos(angle) * dist,
      y: cy + Math.sin(angle) * dist,
    };
    nodeMap.set(f.id, node);
  });

  const links: GLink[] = [];

  // ONLY real relations from database — zero synthetic same-collection clutter
  relations.forEach((rel) => {
    if (nodeMap.has(rel.from_id) && nodeMap.has(rel.to_id)) {
      const relStyle = getRelationStyle(rel.relation);
      links.push({
        id: `rel_${rel.id}`,
        source: rel.from_id,
        target: rel.to_id,
        relation: rel.relation,
        color: relStyle.color,
        isDashed: relStyle.isDashed,
      });
    }
  });

  return { nodes: Array.from(nodeMap.values()), links };
}

// ─── Component ───────────────────────────────────────────────────────────

interface MemoryGraphProps {
  facts: MemoryFactEntry[];
  relations: MemoryRelationEntry[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  selectedRelation: string;
  onSelectFact: (fact: MemoryFactEntry | null, pos?: { x: number; y: number }) => void;
  selectedFactId: string | null;
}

export const MemoryGraph: React.FC<MemoryGraphProps> = ({
  facts,
  relations,
  width,
  height,
  searchQuery,
  selectedCollection,
  selectedRelation,
  onSelectFact,
  selectedFactId,
}) => {
  const fgRef = useRef<ForceGraphMethods<GNode, GLink> | undefined>(undefined);

  const graphData = useMemo(() => {
    if (facts.length === 0 || width === 0) return { nodes: [], links: [] };
    return buildRealDistributedGraph(facts, relations, width, height);
  }, [facts, relations, width, height]);

  // Node visibility helper for filtering & search
  const isNodeVisible = useCallback(
    (node: GNode) => {
      if (!node) return false;
      const sq = searchQuery.trim().toLowerCase();
      const matchesSearch =
        sq.length === 0 ||
        node.label.toLowerCase().includes(sq) ||
        node.factEntry?.fact.toLowerCase().includes(sq);

      if (!matchesSearch) return false;

      if (selectedCollection === "all") return true;
      if (selectedCollection === "Inactive") return node.status === "inactive";
      return node.collection.toLowerCase().includes(selectedCollection.toLowerCase());
    },
    [searchQuery, selectedCollection]
  );

  useEffect(() => {
    const fg = fgRef.current;
    if (!fg) return;

    fg.d3Force("charge", d3Force.forceManyBody<GNode>().strength(-140).distanceMax(450));
    fg.d3Force("center", d3Force.forceCenter(0, 0).strength(0.08));
    fg.d3Force(
      "collide",
      (d3Force.forceCollide as any)().radius((d: GNode) => (d.status === "inactive" ? 6 : 9)).strength(0.75)
    );

    const linkForce = fg.d3Force("link") as d3Force.ForceLink<GNode, GLink> | undefined;
    if (linkForce) {
      linkForce.distance(90).strength(0.35);
    }

    fg.d3ReheatSimulation();

    // Auto-center and fit graph perfectly inside viewport canvas
    const timer = setTimeout(() => {
      if (fgRef.current) {
        fgRef.current.zoomToFit(400, 60);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [graphData, width, height]);

  const paintNode = useCallback(
    (node: NodeObject<GNode>, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const { x = 0, y = 0, status, id, factEntry, color } = node;
      const isSelected = id === selectedFactId;
      const visible = isNodeVisible(node as GNode);

      const r = status === "inactive" ? 3.5 : isSelected ? 8 : 5;
      const opacity = visible ? (isSelected ? 1.0 : status === "inactive" ? 0.4 : 0.85) : 0.04;

      if (!visible && !isSelected) {
        ctx.beginPath();
        ctx.arc(x, y, r, 0, 2 * Math.PI);
        ctx.fillStyle = "rgba(60, 60, 60, 0.06)";
        ctx.fill();
        return;
      }

      // Glowing aura for selected/focused node
      if (isSelected) {
        ctx.beginPath();
        ctx.arc(x, y, r + 7, 0, 2 * Math.PI);
        ctx.fillStyle = `${color}25`;
        ctx.fill();
        ctx.strokeStyle = `${color}90`;
        ctx.lineWidth = 1.2;
        ctx.stroke();
      }

      // Core node dot
      ctx.beginPath();
      ctx.arc(x, y, r, 0, 2 * Math.PI);
      ctx.fillStyle = color;
      ctx.globalAlpha = opacity;
      ctx.fill();
      ctx.globalAlpha = 1.0;

      // Label render on focus / high zoom
      if ((isSelected || globalScale > 2.0) && visible) {
        const fontSize = Math.max(9, Math.min(13, 11 / globalScale));
        ctx.font = `${isSelected ? "bold" : "500"} ${fontSize}px Inter, sans-serif`;
        ctx.fillStyle = isSelected ? "#ffffff" : "rgba(229, 226, 225, 0.85)";
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        const labelText = factEntry.fact.length > 35 ? factEntry.fact.slice(0, 35) + "..." : factEntry.fact;
        ctx.fillText(labelText, x + r + 5, y);
      }
    },
    [isNodeVisible, selectedFactId]
  );

  const paintLink = useCallback(
    (link: LinkObject<GNode, GLink>, ctx: CanvasRenderingContext2D) => {
      const src = link.source as GNode;
      const tgt = link.target as GNode;
      if (!src?.x || !tgt?.x) return;

      const srcVisible = isNodeVisible(src);
      const tgtVisible = isNodeVisible(tgt);

      // Hide or heavily dim edge if filtered out
      const relationMatch =
        selectedRelation === "all" ||
        link.relation?.toUpperCase().includes(selectedRelation.toUpperCase());

      if (!srcVisible || !tgtVisible || !relationMatch) {
        return;
      }

      const sx = src.x ?? 0;
      const sy = src.y ?? 0;
      const tx = tgt.x ?? 0;
      const ty = tgt.y ?? 0;

      ctx.save();
      ctx.beginPath();

      if (link.isDashed) {
        ctx.setLineDash([4, 4]);
      } else {
        ctx.setLineDash([]);
      }

      const mx = (sx + tx) / 2 + (ty - sy) * 0.12;
      const my = (sy + ty) / 2 - (tx - sx) * 0.12;
      ctx.moveTo(sx, sy);
      ctx.quadraticCurveTo(mx, my, tx, ty);

      ctx.strokeStyle = link.color || "#00f2fe";
      ctx.globalAlpha = 0.55;
      ctx.lineWidth = link.isDashed ? 1.0 : 1.3;
      ctx.stroke();
      ctx.restore();
    },
    [isNodeVisible, selectedRelation]
  );

  const handleNodeClick = useCallback(
    (node: NodeObject<GNode>, event: MouseEvent) => {
      if (node.factEntry) {
        const fg = fgRef.current;
        let screenPos = { x: event.clientX, y: event.clientY };
        if (fg && typeof fg.graph2ScreenCoords === "function") {
          const coords = fg.graph2ScreenCoords(node.x ?? 0, node.y ?? 0);
          if (coords && coords.x && coords.y) {
            screenPos = { x: coords.x, y: coords.y };
          }
        }
        onSelectFact(node.factEntry, screenPos);
      } else {
        onSelectFact(null);
      }
    },
    [onSelectFact]
  );

  if (graphData.nodes.length === 0) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 opacity-40">
          <div className="w-5 h-5 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
          <span className="text-[11px] font-mono tracking-widest uppercase text-[rgb(var(--accent))]">
            Loading distributed memory graph...
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="relative w-full h-full">
      {/* Floating Recenter & Fit Button */}
      <div className="absolute top-4 right-4 z-20 pointer-events-auto">
        <button
          onClick={() => fgRef.current?.zoomToFit(400, 60)}
          className="glass-card h-[38px] px-3 rounded-full border border-[rgba(var(--accent),0.18)] bg-[rgba(10,12,14,0.7)] hover:bg-[rgba(10,12,14,0.9)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] backdrop-blur-xl flex items-center gap-2 transition-all duration-200 shadow-xl cursor-pointer"
          title="Recenter and fit memory graph to view"
        >
          <Crosshair size={14} className="text-[rgb(var(--accent))]" />
          <span className="text-[10px] font-mono tracking-wider uppercase text-[rgb(var(--foreground))] font-medium">
            Recenter
          </span>
        </button>
      </div>

      <ForceGraph2D
        ref={fgRef}
        graphData={graphData as any}
        width={width}
        height={height}
        backgroundColor="rgba(0,0,0,0)"
        nodeCanvasObject={paintNode as any}
        nodeCanvasObjectMode={() => "replace"}
        linkCanvasObject={paintLink as any}
        linkCanvasObjectMode={() => "replace"}
        onNodeClick={handleNodeClick as any}
        onBackgroundClick={() => onSelectFact(null)}
        nodePointerAreaPaint={(node: any, color: string, ctx: CanvasRenderingContext2D) => {
          ctx.beginPath();
          ctx.arc(node.x ?? 0, node.y ?? 0, 10, 0, 2 * Math.PI);
          ctx.fillStyle = color;
          ctx.fill();
        }}
        cooldownTicks={140}
        d3AlphaDecay={0.015}
        d3VelocityDecay={0.35}
        enableNodeDrag={true}
        enableZoomInteraction={true}
        minZoom={0.15}
        maxZoom={5}
      />
    </div>
  );
};
