import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useImperativeHandle,
  forwardRef,
  Component,
  ErrorInfo,
  ReactNode,
  useState,
} from "react";
import ForceGraph3D, { ForceGraphMethods } from "react-force-graph-3d";
import * as THREE from "three";
import {
  Heart,
  User,
  Compass,
  BookOpen,
  Box,
  ShieldAlert,
  Archive,
} from "lucide-react";
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
    console.error("[MemoryGraph] WebGL canvas error:", error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="w-full h-full flex items-center justify-center p-6">
          <div className="glass-card max-w-sm w-full p-6 text-center space-y-4 rounded-2xl border border-[rgba(var(--accent),0.2)] bg-[rgb(var(--card))]/95">
            <div className="mx-auto w-12 h-12 rounded-2xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400">
              <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div>
              <h3 className="text-xs font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                WebGL GPU Canvas Error
              </h3>
              <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] mt-1 break-words">
                {this.state.error?.message || "Failed to render WebGL GPU graph context."}
              </p>
            </div>
            <button
              onClick={this.handleRetry}
              className="px-4 py-2 text-[10px] font-mono font-bold uppercase tracking-widest glass-card hover:border-[rgb(var(--accent))]/50 transition-colors cursor-pointer rounded-xl"
            >
              Retry WebGL Context
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
  compactId: string;
  collection: string;
  status: "active" | "inactive";
  topologyNode: MemoryNodeTopology;
  color: string;
  degree: number;
  x?: number;
  y?: number;
  z?: number;
  vx?: number;
  vy?: number;
  vz?: number;
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

// ─── Single Source Color System ───────────────────────────────────────────

export const COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = {
  Identity: {
    main: "#00f2fe",
    glow: "rgba(0, 242, 254, 0.4)",
    text: "#00f2fe",
    desc: "Core identity facts, name, user preferences, and foundational attributes.",
  },
  Profile: {
    main: "#10b981",
    glow: "rgba(16, 185, 129, 0.4)",
    text: "#10b981",
    desc: "Personal background, career history, contacts, and personal metadata.",
  },
  Directives: {
    main: "#c084fc",
    glow: "rgba(192, 132, 252, 0.4)",
    text: "#c084fc",
    desc: "Active operational rules, user instructions, system prompts, and priorities.",
  },
  Narrative: {
    main: "#f43f5e",
    glow: "rgba(244, 63, 94, 0.4)",
    text: "#f43f5e",
    desc: "Temporal story facts, conversation context, historical events, and session logs.",
  },
  Entities: {
    main: "#3b82f6",
    glow: "rgba(59, 130, 246, 0.4)",
    text: "#3b82f6",
    desc: "Projects, codebase modules, tools, software stack, and external references.",
  },
  Constraints: {
    main: "#ef4444",
    glow: "rgba(239, 68, 68, 0.4)",
    text: "#ef4444",
    desc: "Hard system constraints, hardware limits, security bounds, and forbidden rules.",
  },
  Inactive: {
    main: "#64748b",
    glow: "rgba(100, 116, 139, 0.3)",
    text: "#64748b",
    desc: "Historical tombstones and superseded memory facts.",
  },
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
  if (norm.includes("entity") || norm.includes("entities") || norm.includes("project")) return COLLECTION_COLORS.Entities;
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

export function getCollectionIcon(collectionName: string) {
  const norm = collectionName.toLowerCase();
  if (norm.includes("identity")) return Heart;
  if (norm.includes("profile")) return User;
  if (norm.includes("directive")) return Compass;
  if (norm.includes("narrative")) return BookOpen;
  if (norm.includes("entity") || norm.includes("entities")) return Box;
  if (norm.includes("constraint")) return ShieldAlert;
  if (norm.includes("inactive")) return Archive;
  return User;
}

// ─── Graph Data Builder ───────────────────────────────────────────────────

function buildRealDistributedGraph(
  nodes: MemoryNodeTopology[],
  edges: MemoryEdgeTopology[]
): { graphNodes: GNode[]; links: GLink[] } {
  const nodeMap = new Map<string, GNode>();

  const degreeMap = new Map<string, number>();
  nodes.forEach((n) => degreeMap.set(n.id, 0));
  edges.forEach((e) => {
    degreeMap.set(e.from_id, (degreeMap.get(e.from_id) || 0) + 1);
    degreeMap.set(e.to_id, (degreeMap.get(e.to_id) || 0) + 1);
  });

  nodes.forEach((n, idx) => {
    const colPalette = getCollectionColor(n.collection, n.is_superseded);
    const angle = (idx / Math.max(1, nodes.length)) * Math.PI * 2;
    const dist = 50 + Math.random() * 280;

    const compactId = n.id.startsWith("mem_")
      ? `MEM-${n.id.split("_")[1]?.slice(0, 6) || n.id.slice(4, 10)}`
      : n.id;

    const node: GNode = {
      id: n.id,
      label: n.id,
      compactId,
      collection: n.collection,
      status: n.is_superseded ? "inactive" : "active",
      topologyNode: n,
      color: colPalette.main,
      degree: degreeMap.get(n.id) || 0,
      x: Math.cos(angle) * dist,
      y: Math.sin(angle) * dist,
      z: 0,
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

interface ClusterBadgeData {
  collection: string;
  graphX: number;
  graphY: number;
  screenX: number;
  screenY: number;
  factCount: number;
  color: string;
  desc: string;
  activeFacts: number;
  totalRelations: number;
  avgConnections: number;
  outgoingCrossEdges: { targetCollection: string; count: number; relations: Record<string, number> }[];
}

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
    const [clusterBadges, setClusterBadges] = useState<ClusterBadgeData[]>([]);
    const [hoveredBadge, setHoveredBadge] = useState<string | null>(null);
    const [isLayoutStable, setIsLayoutStable] = useState(false);
    const lastBadgeUpdateRef = useRef<number>(0);
    const hasInitialFitRef = useRef(false);

    // Re-trigger layout warmup on node length change
    useEffect(() => {
      if (nodes.length > 0) {
        setIsLayoutStable(false);
        hasInitialFitRef.current = false;
      }
    }, [nodes.length, edges.length]);

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
      const { graphNodes, links } = buildRealDistributedGraph(nodes, edges);
      return { nodes: graphNodes, links };
    }, [nodes, edges, width]);

    // Imperative recenter method
    useImperativeHandle(ref, () => ({
      recenter: () => {
        if (fgRef.current && typeof fgRef.current.zoomToFit === "function") {
          fgRef.current.zoomToFit(600, 100);
        }
      },
    }));

    // Three.js WebGL Custom Node Object Generator (GPU Shaders)
    const createThreeNodeObject = useCallback((node: GNode) => {
      const colPalette = getCollectionColor(node.collection, node.status === "inactive");
      const isSelected = selectedFactId === node.id;
      const isConflict = conflictNodeIds.has(node.id);

      const radius = isSelected ? 12 : node.status === "inactive" ? 3 : Math.min(10, 4 + node.degree * 1.2);
      const mainColor = isConflict ? "#ef4444" : isSelected ? "#00f2fe" : colPalette.main;

      const group = new THREE.Group();

      // Main GPU Sphere Geometry
      const geometry = new THREE.SphereGeometry(radius, 16, 16);
      const material = new THREE.MeshBasicMaterial({
        color: new THREE.Color(mainColor),
        transparent: true,
        opacity: node.status === "inactive" ? 0.4 : 0.9,
      });
      const mesh = new THREE.Mesh(geometry, material);
      group.add(mesh);

      // Hub or Selected Node GPU Halo Ring
      if ((node.degree >= 3 || isSelected || isConflict) && node.status !== "inactive") {
        const ringGeo = new THREE.RingGeometry(radius + 1.5, radius + 3.5, 24);
        const ringMat = new THREE.MeshBasicMaterial({
          color: new THREE.Color(mainColor),
          transparent: true,
          opacity: isSelected ? 0.8 : 0.35,
          side: THREE.DoubleSide,
        });
        const ringMesh = new THREE.Mesh(ringGeo, ringMat);
        group.add(ringMesh);
      }

      return group;
    }, [selectedFactId, conflictNodeIds]);

    // Node visibility check
    const isNodeVisible = useCallback(
      (node: GNode) => {
        if (selectedRelation !== "all") {
          const hasRel = edges.some(
            (e) =>
              (e.from_id === node.id || e.to_id === node.id) &&
              e.relation.toUpperCase().includes(selectedRelation.toUpperCase())
          );
          if (!hasRel) return false;
        }

        const sq = searchQuery.trim().toLowerCase();
        const matchesSearch =
          sq.length === 0 ||
          node.id.toLowerCase().includes(sq) ||
          node.collection.toLowerCase().includes(sq) ||
          (selectedFactDetail?.id === node.id &&
            (selectedFactDetail?.fact.toLowerCase().includes(sq) ||
              selectedFactDetail?.session_id.toLowerCase().includes(sq)));

        if (!matchesSearch) return false;

        if (selectedCollection === "all") return true;
        if (selectedCollection === "Inactive") return node.status === "inactive";
        return node.collection.toLowerCase().includes(selectedCollection.toLowerCase());
      },
      [searchQuery, selectedCollection, selectedRelation, edges, selectedFactDetail]
    );

    // Update screen coordinates for Landmark Badges (Subpanel 1)
    const updateCentroidBadges = useCallback(() => {
      const now = performance.now();
      if (now - lastBadgeUpdateRef.current < 80) return;
      lastBadgeUpdateRef.current = now;

      const fg = fgRef.current;
      if (!fg || !isLayoutStable) return;

      const groups = new Map<string, { nodes: GNode[]; color: string }>();

      graphData.nodes.forEach((n) => {
        if (!isNodeVisible(n)) return;
        const col = n.collection || "Identity";
        if (!groups.has(col)) {
          groups.set(col, { nodes: [], color: n.color });
        }
        groups.get(col)!.nodes.push(n);
      });

      const badges: ClusterBadgeData[] = [];

      groups.forEach((data, colName) => {
        if (data.nodes.length === 0) return;

        let sumX = 0;
        let sumY = 0;
        data.nodes.forEach((n) => {
          sumX += n.x ?? 0;
          sumY += n.y ?? 0;
        });

        const gx = sumX / data.nodes.length;
        const gy = sumY / data.nodes.length;

        const screenPos = fg.graph2ScreenCoords(gx, gy, 0);
        if (!screenPos || isNaN(screenPos.x) || isNaN(screenPos.y)) return;

        const palette = COLLECTION_COLORS[colName] || COLLECTION_COLORS.Identity;

        // Calculate Subpanel 1 metrics
        const totalRels = edges.filter((e) =>
          data.nodes.some((n) => n.id === e.from_id || n.id === e.to_id)
        ).length;
        const avgConn = data.nodes.length > 0 ? (totalRels / data.nodes.length).toFixed(2) : "0.00";

        badges.push({
          collection: colName,
          graphX: gx,
          graphY: gy,
          screenX: screenPos.x,
          screenY: screenPos.y,
          factCount: data.nodes.length,
          color: data.color,
          desc: palette.desc,
          activeFacts: data.nodes.filter((n) => n.status === "active").length,
          totalRelations: totalRels,
          avgConnections: Number(avgConn),
          outgoingCrossEdges: [],
        });
      });

      setClusterBadges(badges);
    }, [graphData.nodes, edges, isNodeVisible, isLayoutStable]);

    // Disable 3D rotation and attach OrbitControls camera change listener for smooth badge tracking
    useEffect(() => {
      if (!fgRef.current) return;
      const controls = fgRef.current.controls() as any;
      if (!controls) return;

      controls.enableRotate = false;
      if (THREE.MOUSE) {
        controls.mouseButtons = {
          LEFT: THREE.MOUSE.PAN,
          MIDDLE: THREE.MOUSE.DOLLY,
          RIGHT: THREE.MOUSE.PAN,
        };
      }
      const handleChange = () => updateCentroidBadges();
      controls.addEventListener("change", handleChange);
      return () => {
        controls.removeEventListener("change", handleChange);
      };
    }, [updateCentroidBadges]);

    // Periodically update landmark badges when layout is stable
    useEffect(() => {
      if (isLayoutStable) {
        updateCentroidBadges();
      }
    }, [isLayoutStable, updateCentroidBadges]);

    // Camera settling event handler
    const handleEngineStop = useCallback(() => {
      setIsLayoutStable(true);
      if (!hasInitialFitRef.current) {
        hasInitialFitRef.current = true;
        if (fgRef.current && typeof fgRef.current.zoomToFit === "function") {
          fgRef.current.zoomToFit(600, 100);
        }
      }
    }, []);

    // WebGL Node Click Handler
    const handleNodeClick = useCallback(
      (node: GNode) => {
        if (!node || !node.id) return;
        const fg = fgRef.current;
        if (fg) {
          const screenPos = fg.graph2ScreenCoords(node.x ?? 0, node.y ?? 0, 0);
          onSelectNode(node.id, screenPos ? { x: screenPos.x, y: screenPos.y } : undefined);
        } else {
          onSelectNode(node.id);
        }
      },
      [onSelectNode]
    );

    return (
      <GraphErrorBoundary>
        <div className="relative w-full h-full">
          {/* Subpanel 6: Initial WebGL Graph Loader Overlay (No Card Wrapper) */}
          {!isLayoutStable && (
            <div className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-[rgb(var(--background))]/95 backdrop-blur-3xl transition-opacity duration-300 pointer-events-none select-none">
              {/* Glowing Geometric Polyhedron Icon with Concentric Ambient Pulse Rings */}
              <div className="relative flex items-center justify-center w-36 h-36 mb-6">
                <div className="absolute inset-0 rounded-full bg-[rgb(var(--accent))]/10 animate-ping duration-1000" />
                <div className="absolute inset-4 rounded-full border border-[rgb(var(--accent))]/25 animate-pulse" />
                <div className="absolute inset-8 rounded-full bg-[rgb(var(--accent))]/10 blur-xl animate-pulse" />
                <div className="relative z-10 w-20 h-20 rounded-3xl bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 flex items-center justify-center text-[rgb(var(--accent))] shadow-[0_0_40px_rgba(var(--accent),0.4)]">
                  <Box size={40} className="animate-pulse" />
                </div>
              </div>

              {/* Text Stack (Subpanel 6 Image Spec) */}
              <div className="flex flex-col items-center gap-2 text-center">
                <h3 className="text-[14px] font-mono font-black tracking-[0.2em] text-[rgb(var(--foreground))] uppercase">
                  Building memory graph...
                </h3>
                <div className="px-4 py-1.5 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] shadow-md">
                  <span className="text-[12px] font-mono font-bold text-[rgb(var(--accent))]">
                    {nodes.length.toLocaleString()} nodes · {edges.length.toLocaleString()} edges
                  </span>
                </div>
                <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] mt-1">
                  Optimizing layout and relationships
                </p>
              </div>
            </div>
          )}

          {/* WebGL GPU Graph Container */}
          <div className={isLayoutStable ? "opacity-100 transition-opacity duration-300 w-full h-full" : "opacity-0 w-full h-full"}>
            <ForceGraph3D
              ref={fgRef}
              graphData={graphData as any}
              width={width}
              height={height}
              numDimensions={2}
              backgroundColor="rgba(0,0,0,0)"
              nodeThreeObject={createThreeNodeObject as any}
              linkColor={(link: any) => link.color || "#64748b"}
              linkWidth={1.2}
              linkOpacity={0.4}
              onNodeClick={handleNodeClick as any}
              onBackgroundClick={() => onSelectNode(null)}
              onEngineStop={handleEngineStop}
              warmupTicks={50}
              cooldownTicks={100}
              enableNavigationControls={true}
            />
          </div>

          {/* Floating Centroid Cluster Landmark Badges Layer */}
          {isLayoutStable && (
            <div className="absolute inset-0 pointer-events-none overflow-hidden z-10">
              {clusterBadges.map((badge) => {
                const IconComp = getCollectionIcon(badge.collection);
                const isHovered = hoveredBadge === badge.collection;

                return (
                  <div
                    key={badge.collection}
                    style={{
                      left: `${badge.screenX}px`,
                      top: `${badge.screenY}px`,
                      transform: "translate(-50%, -50%)",
                    }}
                    className="absolute pointer-events-auto transition-transform duration-100"
                    onMouseEnter={() => setHoveredBadge(badge.collection)}
                    onMouseLeave={() => setHoveredBadge(null)}
                  >
                    {/* Clean Landmark Badge Pill */}
                    <div
                      className="flex items-center gap-2.5 px-3.5 py-2 rounded-full glass-card border border-[rgba(var(--accent),0.2)] bg-[rgb(var(--card))]/90 backdrop-blur-2xl shadow-xl hover:border-[rgb(var(--accent))]/50 transition-all cursor-pointer group select-none"
                      style={{
                        boxShadow: isHovered ? `0 0 24px ${badge.color}50` : undefined,
                      }}
                    >
                      <IconComp size={15} style={{ color: badge.color }} className="shrink-0" />
                      <span className="text-[12px] font-mono font-bold tracking-wide text-[rgb(var(--foreground))] uppercase">
                        {badge.collection}
                      </span>
                      <span
                        className="text-[11px] font-mono font-bold px-2 py-0.5 rounded-full"
                        style={{ backgroundColor: `${badge.color}25`, color: badge.color }}
                      >
                        {badge.factCount}
                      </span>
                    </div>

                    {/* Subpanel 1: Landmark Badge Hover Tooltip (LEGEND - EXPANDED COLLECTION VIEW) */}
                    {isHovered && (
                      <div className="absolute left-1/2 -translate-x-1/2 top-full mt-3 w-[290px] p-4 rounded-2xl glass-card border border-[rgba(var(--accent),0.25)] bg-[rgb(var(--card))]/95 backdrop-blur-2xl shadow-2xl z-30 flex flex-col gap-3 pointer-events-none text-[rgb(var(--foreground))]">
                        {/* Header */}
                        <div className="flex items-start justify-between border-b border-[rgba(var(--border),0.15)] pb-2.5">
                          <div className="flex items-center gap-2">
                            <IconComp size={16} style={{ color: badge.color }} />
                            <span className="text-[12px] font-mono font-bold uppercase text-[rgb(var(--foreground))]">
                              {badge.collection}
                            </span>
                            <span
                              className="text-[10px] font-mono font-bold px-2 py-0.5 rounded-full"
                              style={{ backgroundColor: `${badge.color}25`, color: badge.color }}
                            >
                              {badge.factCount}
                            </span>
                          </div>
                        </div>

                        {/* Collection Description */}
                        <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] leading-relaxed">
                          {badge.desc}
                        </p>

                        {/* 2x2 Metric Grid (Subpanel 1) */}
                        <div className="grid grid-cols-2 gap-2 pt-1 border-t border-[rgba(var(--border),0.15)]">
                          <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                            <span className="text-[9px] font-mono font-bold uppercase text-[rgb(var(--foreground-muted))] block">
                              Active Facts
                            </span>
                            <span className="text-[13px] font-mono font-bold text-[rgb(var(--foreground))]">
                              {badge.activeFacts}
                            </span>
                          </div>

                          <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                            <span className="text-[9px] font-mono font-bold uppercase text-[rgb(var(--foreground-muted))] block">
                              Total Relations
                            </span>
                            <span className="text-[13px] font-mono font-bold text-[rgb(var(--accent))]">
                              {badge.totalRelations}
                            </span>
                          </div>

                          <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                            <span className="text-[9px] font-mono font-bold uppercase text-[rgb(var(--foreground-muted))] block">
                              Avg. Connections
                            </span>
                            <span className="text-[13px] font-mono font-bold text-[rgb(var(--foreground))]">
                              {badge.avgConnections}
                            </span>
                          </div>

                          <div className="p-2 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                            <span className="text-[9px] font-mono font-bold uppercase text-[rgb(var(--foreground-muted))] block">
                              Last Updated
                            </span>
                            <span className="text-[11px] font-mono font-bold text-emerald-400">
                              Just now
                            </span>
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </GraphErrorBoundary>
    );
  }
);

MemoryGraph.displayName = "MemoryGraph";
