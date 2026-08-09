import React, { useCallback, useEffect, useMemo, useRef } from "react";
import ForceGraph2D, { ForceGraphMethods, NodeObject, LinkObject } from "react-force-graph-2d";
import * as d3Force from "d3-force";
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
}

// ─── Collection Color Palette ─────────────────────────────────────────────

export const COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string }> = {
  Identity: { main: "#00f2fe", glow: "rgba(0, 242, 254, 0.4)", text: "#00f2fe" },
  Profile: { main: "#10b981", glow: "rgba(16, 185, 129, 0.4)", text: "#10b981" },
  Directives: { main: "#a855f7", glow: "rgba(168, 85, 247, 0.4)", text: "#a855f7" },
  Constraints: { main: "#f59e0b", glow: "rgba(245, 158, 11, 0.4)", text: "#f59e0b" },
  Entities: { main: "#ef4444", glow: "rgba(239, 68, 68, 0.4)", text: "#ef4444" },
};

const DEFAULT_COLOR = { main: "#3b82f6", glow: "rgba(59, 130, 246, 0.4)", text: "#3b82f6" };

export function getCollectionColor(rawCollection: string) {
  const norm = rawCollection.toLowerCase();
  if (norm.includes("identity")) return COLLECTION_COLORS.Identity;
  if (norm.includes("profile")) return COLLECTION_COLORS.Profile;
  if (norm.includes("directive")) return COLLECTION_COLORS.Directives;
  if (norm.includes("constraint")) return COLLECTION_COLORS.Constraints;
  if (norm.includes("entity") || norm.includes("project")) return COLLECTION_COLORS.Entities;
  return DEFAULT_COLOR;
}

export const RELATION_COLORS: Record<string, string> = {
  restricted_by: "#ef4444",
  DEPENDS_ON: "#f59e0b",
  SHAPES: "#a855f7",
  SUPPORTS: "#10b981",
  SUPERSEDES: "#6b7280",
};

// ─── Graph Data Builder ───────────────────────────────────────────────────

function buildDistributedGraph(
  facts: MemoryFactEntry[],
  relations: MemoryRelationEntry[],
  width: number,
  height: number
): { nodes: GNode[]; links: GLink[] } {
  const cx = width / 2;
  const cy = height / 2;

  const nodeMap = new Map<string, GNode>();

  facts.forEach((f, idx) => {
    const colPalette = getCollectionColor(f.collection);
    const angle = (idx / facts.length) * Math.PI * 2;
    const radius = 80 + Math.random() * 220;

    const node: GNode = {
      id: f.id,
      label: f.fact,
      collection: f.collection,
      status: f.is_superseded ? "inactive" : "active",
      factEntry: f,
      color: colPalette.main,
      x: cx + Math.cos(angle) * radius,
      y: cy + Math.sin(angle) * radius,
    };
    nodeMap.set(f.id, node);
  });

  const links: GLink[] = [];

  relations.forEach((rel) => {
    if (nodeMap.has(rel.from_id) && nodeMap.has(rel.to_id)) {
      links.push({
        id: `rel_${rel.id}`,
        source: rel.from_id,
        target: rel.to_id,
        relation: rel.relation,
        color: RELATION_COLORS[rel.relation] || "#00f2fe",
      });
    }
  });

  const collectionGroups = new Map<string, GNode[]>();
  nodeMap.forEach((n) => {
    const list = collectionGroups.get(n.collection) || [];
    list.push(n);
    collectionGroups.set(n.collection, list);
  });

  collectionGroups.forEach((nodes, col) => {
    for (let i = 0; i < nodes.length - 1; i += 3) {
      if (nodes[i + 1]) {
        links.push({
          id: `cluster_${col}_${i}`,
          source: nodes[i].id,
          target: nodes[i + 1].id,
          relation: "SAME_COLLECTION",
          color: "rgba(255, 255, 255, 0.04)",
        });
      }
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
  onSelectFact,
  selectedFactId,
}) => {
  const fgRef = useRef<ForceGraphMethods<GNode, GLink> | undefined>(undefined);

  const graphData = useMemo(() => {
    if (facts.length === 0 || width === 0) return { nodes: [], links: [] };
    return buildDistributedGraph(facts, relations, width, height);
  }, [facts, relations, width, height]);

  useEffect(() => {
    const fg = fgRef.current;
    if (!fg) return;

    fg.d3Force("charge", d3Force.forceManyBody<GNode>().strength(-120).distanceMax(400));
    fg.d3Force("center", d3Force.forceCenter(width / 2, height / 2).strength(0.04));
    fg.d3Force(
      "collide",
      (d3Force.forceCollide as any)().radius((d: GNode) => (d.status === "inactive" ? 5 : 8)).strength(0.7)
    );

    const linkForce = fg.d3Force("link") as d3Force.ForceLink<GNode, GLink> | undefined;
    if (linkForce) {
      linkForce
        .distance((l: GLink) => (l.relation === "SAME_COLLECTION" ? 45 : 90))
        .strength((l: GLink) => (l.relation === "SAME_COLLECTION" ? 0.25 : 0.4));
    }

    fg.d3ReheatSimulation();
  }, [graphData, width, height]);

  const paintNode = useCallback(
    (node: NodeObject<GNode>, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const { x = 0, y = 0, status, id, factEntry, color } = node;
      const isSelected = id === selectedFactId;
      const sq = searchQuery.toLowerCase();
      const matchesSearch =
        sq.length > 1 && (node.label?.toLowerCase().includes(sq) || factEntry?.fact.toLowerCase().includes(sq));

      const colMatch =
        selectedCollection === "all" ||
        factEntry?.collection.toLowerCase().includes(selectedCollection.toLowerCase());

      const dimmed = !colMatch || (sq.length > 1 && !matchesSearch);

      const r = status === "inactive" ? 3.5 : isSelected ? 8 : matchesSearch ? 7 : 5;
      const opacity = dimmed ? 0.08 : status === "inactive" ? 0.3 : isSelected ? 1.0 : matchesSearch ? 0.95 : 0.8;

      if (isSelected || matchesSearch) {
        ctx.beginPath();
        ctx.arc(x, y, r + 6, 0, 2 * Math.PI);
        ctx.fillStyle = `${color}25`;
        ctx.fill();
        ctx.strokeStyle = `${color}80`;
        ctx.lineWidth = 1;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(x, y, r, 0, 2 * Math.PI);
      ctx.fillStyle = dimmed ? "rgba(100, 100, 100, 0.15)" : color;
      ctx.globalAlpha = opacity;
      ctx.fill();
      ctx.globalAlpha = 1.0;

      if ((isSelected || matchesSearch || globalScale > 2.2) && !dimmed) {
        const fontSize = Math.max(9, Math.min(13, 11 / globalScale));
        ctx.font = `${isSelected ? "bold" : "500"} ${fontSize}px Inter, sans-serif`;
        ctx.fillStyle = isSelected ? "#ffffff" : "rgba(229, 226, 225, 0.85)";
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        const labelText = factEntry.fact.length > 32 ? factEntry.fact.slice(0, 32) + "..." : factEntry.fact;
        ctx.fillText(labelText, x + r + 5, y);
      }
    },
    [searchQuery, selectedCollection, selectedFactId]
  );

  const paintLink = useCallback(
    (link: LinkObject<GNode, GLink>, ctx: CanvasRenderingContext2D) => {
      const src = link.source as GNode;
      const tgt = link.target as GNode;
      if (!src?.x || !tgt?.x) return;

      const sx = src.x ?? 0;
      const sy = src.y ?? 0;
      const tx = tgt.x ?? 0;
      const ty = tgt.y ?? 0;

      ctx.beginPath();
      if (link.relation !== "SAME_COLLECTION") {
        const mx = (sx + tx) / 2 + (ty - sy) * 0.15;
        const my = (sy + ty) / 2 - (tx - sx) * 0.15;
        ctx.moveTo(sx, sy);
        ctx.quadraticCurveTo(mx, my, tx, ty);
        ctx.strokeStyle = link.color || "rgba(0, 219, 233, 0.35)";
        ctx.lineWidth = 1.2;
      } else {
        ctx.moveTo(sx, sy);
        ctx.lineTo(tx, ty);
        ctx.strokeStyle = "rgba(255, 255, 255, 0.04)";
        ctx.lineWidth = 0.5;
      }
      ctx.stroke();
    },
    []
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
            Loading memory graph...
          </span>
        </div>
      </div>
    );
  }

  return (
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
  );
};
