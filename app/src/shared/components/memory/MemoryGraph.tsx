import { useCallback, useEffect, useMemo, useRef, useImperativeHandle, forwardRef, Component, ErrorInfo, ReactNode } from "react";
import ForceGraph2D, { ForceGraphMethods, NodeObject, LinkObject } from "react-force-graph-2d";
import * as d3Force from "d3-force";
import { MemoryNodeTopology, MemoryEdgeTopology, MemoryFactDetail } from "@/services/memoryService";

interface GraphErrorBoundaryProps {
  children: ReactNode;
}

interface GraphErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class GraphErrorBoundary extends Component<GraphErrorBoundaryProps, GraphErrorBoundaryState> {
  constructor(props: GraphErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[MemoryGraph] Canvas/WebGL render error:", error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="w-full h-full flex items-center justify-center p-6">
          <div className="glass-card max-w-sm w-full p-6 text-center space-y-4">
            <div className="mx-auto w-12 h-12 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400">
              <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div>
              <h3 className="text-xs font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                Knowledge Graph Canvas Error
              </h3>
              <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] mt-1 break-words">
                {this.state.error?.message || "Failed to render 2D force graph canvas."}
              </p>
            </div>
            <button
              onClick={this.handleRetry}
              className="px-4 py-2 text-[10px] font-mono font-bold uppercase tracking-widest glass-card hover:border-[rgb(var(--accent))]/50 transition-colors cursor-pointer"
            >
              Retry Canvas Render
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export interface GNode {
  id: string;
  label: string;
  collection: string;
  status: "active" | "inactive";
  topologyNode: MemoryNodeTopology;
  factDetail?: MemoryFactDetail | null;
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

export interface MemoryGraphRef {
  recenter: () => void;
}

// ─── Collection & Relation Color System ───────────────────────────────────

export const COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string }> = {
  Identity: { main: "#00f2fe", glow: "rgba(0, 242, 254, 0.4)", text: "#00f2fe" },
  Profile: { main: "#10b981", glow: "rgba(16, 185, 129, 0.4)", text: "#10b981" },
  Directives: { main: "#c084fc", glow: "rgba(192, 132, 252, 0.4)", text: "#c084fc" },
  Narrative: { main: "#fbbf24", glow: "rgba(251, 191, 36, 0.4)", text: "#fbbf24" },
  Entities: { main: "#f43f5e", glow: "rgba(244, 63, 94, 0.4)", text: "#f43f5e" },
  Constraints: { main: "#ef4444", glow: "rgba(239, 68, 68, 0.4)", text: "#ef4444" },
  Inactive: { main: "#64748b", glow: "rgba(100, 116, 139, 0.3)", text: "#64748b" },
};

export const RELATION_STYLES: Record<string, { color: string; isDashed: boolean }> = {
  SUPPORTS: { color: "#10b981", isDashed: false },
  SUPERSEDES: { color: "#00f2fe", isDashed: false },
  SHAPES: { color: "#c084fc", isDashed: false },
  DEPENDS_ON: { color: "#fbbf24", isDashed: false },
  CONFLICTS_WITH: { color: "#ef4444", isDashed: true },
  restricted_by: { color: "#f43f5e", isDashed: true },
  OTHER: { color: "#64748b", isDashed: true },
};

export function getCollectionColor(rawCollection: string, isSuperseded = false) {
  if (isSuperseded) return COLLECTION_COLORS.Inactive;
  const norm = rawCollection.toLowerCase();
  if (norm.includes("identity")) return COLLECTION_COLORS.Identity;
  if (norm.includes("profile")) return COLLECTION_COLORS.Profile;
  if (norm.includes("directive")) return COLLECTION_COLORS.Directives;
  if (norm.includes("narrative") || norm.includes("context")) return COLLECTION_COLORS.Narrative;
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

// ─── Graph Data Builder ───────────────────────────────────────────────────

function buildRealDistributedGraph(
  nodes: MemoryNodeTopology[],
  edges: MemoryEdgeTopology[],
  selectedDetail?: MemoryFactDetail | null
): { graphNodes: GNode[]; links: GLink[] } {
  const nodeMap = new Map<string, GNode>();

  nodes.forEach((n, idx) => {
    const colPalette = getCollectionColor(n.collection, n.is_superseded);
    const angle = (idx / Math.max(1, nodes.length)) * Math.PI * 2;
    const dist = 40 + Math.random() * 260;

    const detail = selectedDetail?.id === n.id ? selectedDetail : undefined;

    const node: GNode = {
      id: n.id,
      label: n.id,
      collection: n.collection,
      status: n.is_superseded ? "inactive" : "active",
      topologyNode: n,
      factDetail: detail,
      color: colPalette.main,
      x: Math.cos(angle) * dist,
      y: Math.sin(angle) * dist,
    };
    nodeMap.set(n.id, node);
  });

  const links: GLink[] = [];

  edges.forEach((rel) => {
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

  return { graphNodes: Array.from(nodeMap.values()), links };
}

// ─── Component ───────────────────────────────────────────────────────────

interface MemoryGraphProps {
  nodes: MemoryNodeTopology[];
  edges: MemoryEdgeTopology[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  selectedRelation: string;
  onSelectNode: (nodeId: string | null, pos?: { x: number; y: number }) => void;
  selectedFactId: string | null;
  selectedFactDetail?: MemoryFactDetail | null;
  conflictPairs?: { fact_a: MemoryNodeTopology; fact_b: MemoryNodeTopology }[];
}

export const MemoryGraph = forwardRef<MemoryGraphRef, MemoryGraphProps>(
  (
    {
      nodes,
      edges,
      width,
      height,
      searchQuery,
      selectedCollection,
      selectedRelation,
      onSelectNode,
      selectedFactId,
      selectedFactDetail,
      conflictPairs = [],
    },
    ref
  ) => {
    const fgRef = useRef<ForceGraphMethods<GNode, GLink> | undefined>(undefined);

    const conflictNodeIds = useMemo(() => {
      const set = new Set<string>();
      conflictPairs.forEach((pair) => {
        set.add(pair.fact_a.id);
        set.add(pair.fact_b.id);
      });
      return set;
    }, [conflictPairs]);

    const graphData = useMemo(() => {
      if (nodes.length === 0 || width === 0) return { nodes: [], links: [] };
      const { graphNodes, links } = buildRealDistributedGraph(nodes, edges, selectedFactDetail);
      return { nodes: graphNodes, links };
    }, [nodes, edges, width, selectedFactDetail]);

    // Imperative recenter method
    useImperativeHandle(ref, () => ({
      recenter: () => {
        if (fgRef.current && typeof fgRef.current.zoomToFit === "function") {
          fgRef.current.zoomToFit(400, 60);
        }
      },
    }));

    // Auto-recenter on mount or data update
    useEffect(() => {
      if (graphData.nodes.length === 0) return;
      const timer = setTimeout(() => {
        if (fgRef.current && typeof fgRef.current.zoomToFit === "function") {
          fgRef.current.zoomToFit(400, 60);
        }
      }, 300);
      return () => clearTimeout(timer);
    }, [graphData.nodes.length]);

    const isNodeVisible = useCallback(
      (node: GNode) => {
        if (!node) return false;

        // In Unresolved Conflicts Mode, isolate conflict node pairs if conflicts exist
        if (conflictPairs.length > 0) {
          return conflictNodeIds.has(node.id);
        }

        const sq = searchQuery.trim().toLowerCase();
        const matchesSearch =
          sq.length === 0 ||
          node.id.toLowerCase().includes(sq) ||
          node.collection.toLowerCase().includes(sq) ||
          node.factDetail?.fact.toLowerCase().includes(sq) ||
          node.factDetail?.session_id.toLowerCase().includes(sq);

        if (!matchesSearch) return false;

        if (selectedCollection === "all") return true;
        if (selectedCollection === "Inactive") return node.status === "inactive";
        return node.collection.toLowerCase().includes(selectedCollection.toLowerCase());
      },
      [searchQuery, selectedCollection, conflictPairs, conflictNodeIds]
    );

    useEffect(() => {
      const fg = fgRef.current;
      if (!fg) return;

      fg.d3Force("charge", d3Force.forceManyBody<GNode>().strength(-140).distanceMax(450));
      fg.d3Force("center", d3Force.forceCenter(0, 0).strength(0.04));
      fg.d3Force(
        "collide",
        (d3Force.forceCollide as any)().radius((d: GNode) => (d.status === "inactive" ? 6 : 9)).strength(0.75)
      );

      const linkForce = fg.d3Force("link") as d3Force.ForceLink<GNode, GLink> | undefined;
      if (linkForce) {
        linkForce.distance(90).strength(0.35);
      }

      fg.d3ReheatSimulation();
    }, [graphData, width, height]);

    const paintNode = useCallback(
      (node: NodeObject<GNode>, ctx: CanvasRenderingContext2D, globalScale: number) => {
        const { x = 0, y = 0, status, id, collection, color, factDetail } = node;
        const isSelected = id === selectedFactId;
        const isConflict = conflictNodeIds.has(id);
        const visible = isNodeVisible(node as GNode);

        const r = status === "inactive" ? 3.5 : isSelected ? 8 : isConflict ? 7 : 5;
        const opacity = visible ? (isSelected ? 1.0 : status === "inactive" ? 0.4 : 0.85) : 0.04;

        if (!visible && !isSelected) {
          ctx.beginPath();
          ctx.arc(x, y, r, 0, 2 * Math.PI);
          ctx.fillStyle = "rgba(60, 60, 60, 0.06)";
          ctx.fill();
          return;
        }

        // Conflict Pulse Glow
        if (isConflict && visible) {
          ctx.beginPath();
          ctx.arc(x, y, r + 5, 0, 2 * Math.PI);
          ctx.fillStyle = "rgba(239, 68, 68, 0.25)";
          ctx.fill();
          ctx.strokeStyle = "#ef4444";
          ctx.lineWidth = 1.0;
          ctx.stroke();
        }

        // Selection Ring Glow
        if (isSelected) {
          ctx.beginPath();
          ctx.arc(x, y, r + 7, 0, 2 * Math.PI);
          ctx.fillStyle = `${color}25`;
          ctx.fill();
          ctx.strokeStyle = `${color}90`;
          ctx.lineWidth = 1.2;
          ctx.stroke();
        }

        // Main Node Circle
        ctx.beginPath();
        ctx.arc(x, y, r, 0, 2 * Math.PI);
        ctx.fillStyle = isConflict ? "#ef4444" : color;
        ctx.globalAlpha = opacity;
        ctx.fill();
        ctx.globalAlpha = 1.0;

        // Render Compact Fact ID badge (e.g. MEM-1024) and Collection Pill
        if ((isSelected || globalScale > 1.8) && visible) {
          const fontSize = Math.max(9, Math.min(13, 11 / globalScale));
          ctx.font = `bold ${fontSize}px Inter, sans-serif`;
          ctx.fillStyle = isSelected ? "#ffffff" : "rgba(229, 226, 225, 0.90)";
          ctx.textAlign = "left";
          ctx.textBaseline = "middle";

          // Format ID cleanly as compact ID (e.g. MEM-1024 or upper id)
          const compactId = id.startsWith("mem_")
            ? `MEM-${id.split("_")[1]?.slice(0, 6) || id.slice(4, 10)}`
            : id;

          const badgeText = `${compactId} · ${collection.toUpperCase()}`;
          ctx.fillText(badgeText, x + r + 6, y);

          // If detail text is loaded and selected, draw snippet preview below
          if (isSelected && factDetail?.fact) {
            ctx.font = `normal ${fontSize - 1}px Inter, sans-serif`;
            ctx.fillStyle = "rgba(180, 180, 180, 0.75)";
            const snippet = factDetail.fact.length > 30 ? factDetail.fact.slice(0, 30) + "..." : factDetail.fact;
            ctx.fillText(`"${snippet}"`, x + r + 6, y + fontSize + 2);
          }
        }
      },
      [isNodeVisible, selectedFactId, conflictNodeIds]
    );

    const paintLink = useCallback(
      (link: LinkObject<GNode, GLink>, ctx: CanvasRenderingContext2D) => {
        const src = link.source as GNode;
        const tgt = link.target as GNode;
        if (!src?.x || !tgt?.x) return;

        const srcVisible = isNodeVisible(src);
        const tgtVisible = isNodeVisible(tgt);

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
        if (node.topologyNode) {
          const fg = fgRef.current;
          let screenPos = { x: event.clientX, y: event.clientY };
          if (fg && typeof fg.graph2ScreenCoords === "function") {
            const coords = fg.graph2ScreenCoords(node.x ?? 0, node.y ?? 0);
            if (coords && coords.x && coords.y) {
              screenPos = { x: coords.x, y: coords.y };
            }
          }
          onSelectNode(node.topologyNode.id, screenPos);
        } else {
          onSelectNode(null);
        }
      },
      [onSelectNode]
    );

    if (nodes.length === 0) {
      return (
        <div className="w-full h-full flex items-center justify-center">
          <div className="flex flex-col items-center gap-3 opacity-40">
            <div className="w-5 h-5 border border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
            <span className="text-[11px] font-mono tracking-widest uppercase text-[rgb(var(--accent))]">
              Loading memory topology graph...
            </span>
          </div>
        </div>
      );
    }

    return (
      <GraphErrorBoundary>
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
          onBackgroundClick={() => onSelectNode(null)}
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
      </GraphErrorBoundary>
    );
  }
);

MemoryGraph.displayName = "MemoryGraph";
