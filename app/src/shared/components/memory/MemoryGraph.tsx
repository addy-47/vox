import React, {
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
import { AnimatePresence, motion } from "framer-motion";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import {
  Heart,
  User,
  Compass,
  BookOpen,
  Box,
  ShieldAlert,
  Archive,
  X,
  Sparkles,
} from "lucide-react";
import { MemoryNodeTopology, MemoryEdgeTopology, MemoryFactDetail } from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";

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
          <div className="glass-card max-w-sm w-full p-6 text-center space-y-4 rounded-2xl border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-[20px] shadow-2xl">
            <div className="mx-auto w-12 h-12 rounded-2xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400">
              <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div>
              <h3 className="text-[13px] font-sans font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                WebGL GPU Canvas Error
              </h3>
              <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] mt-1 break-words">
                {this.state.error?.message || "Failed to render WebGL GPU graph context."}
              </p>
            </div>
            <button
              onClick={this.handleRetry}
              className="px-4 py-2 text-[11px] font-mono font-bold uppercase tracking-widest glass-card hover:border-[rgb(var(--accent))]/50 transition-colors cursor-pointer rounded-xl"
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
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
}

export interface GLink {
  id: string;
  sourceIndex: number;
  targetIndex: number;
  fromId: string;
  toId: string;
  relation: string;
  color: string;
  isDashed: boolean;
}

export interface MemoryGraphRef {
  recenter: () => void;
}

// Persistent node position cache map
const nodePosCache = new Map<string, { x: number; y: number; z: number }>();

// Theme-Aware Vibrant Color Palettes (Dark vs Light contrast optimized)
export const DARK_COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = {
  Identity: {
    main: "#38bdf8",
    glow: "rgba(56, 189, 248, 0.4)",
    text: "#38bdf8",
    desc: "Core identity facts, user name, preferences, and foundational attributes.",
  },
  Profile: {
    main: "#34d399",
    glow: "rgba(52, 211, 153, 0.4)",
    text: "#34d399",
    desc: "Personal background, career history, contacts, and personal metadata.",
  },
  Directives: {
    main: "#a78bfa",
    glow: "rgba(167, 139, 250, 0.4)",
    text: "#a78bfa",
    desc: "Active operational rules, user instructions, system prompts, and priorities.",
  },
  Narrative: {
    main: "#f472b6",
    glow: "rgba(244, 114, 182, 0.4)",
    text: "#f472b6",
    desc: "Temporal story facts, conversation context, historical events, and session logs.",
  },
  Entities: {
    main: "#facc15",
    glow: "rgba(250, 204, 21, 0.4)",
    text: "#facc15",
    desc: "Projects, codebase modules, tools, software stack, and external references.",
  },
  Constraints: {
    main: "#f43f5e",
    glow: "rgba(244, 63, 94, 0.4)",
    text: "#f43f5e",
    desc: "Hard system constraints, hardware limits, security bounds, and forbidden rules.",
  },
  Inactive: {
    main: "#64748b",
    glow: "rgba(100, 116, 139, 0.3)",
    text: "#64748b",
    desc: "Historical tombstones and superseded memory facts.",
  },
};

export const LIGHT_COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = {
  Identity: {
    main: "#0284c7",
    glow: "rgba(2, 132, 199, 0.35)",
    text: "#0284c7",
    desc: "Core identity facts, user name, preferences, and foundational attributes.",
  },
  Profile: {
    main: "#059669",
    glow: "rgba(5, 150, 105, 0.35)",
    text: "#059669",
    desc: "Personal background, career history, contacts, and personal metadata.",
  },
  Directives: {
    main: "#7c3aed",
    glow: "rgba(124, 58, 237, 0.35)",
    text: "#7c3aed",
    desc: "Active operational rules, user instructions, system prompts, and priorities.",
  },
  Narrative: {
    main: "#db2777",
    glow: "rgba(219, 39, 119, 0.35)",
    text: "#db2777",
    desc: "Temporal story facts, conversation context, historical events, and session logs.",
  },
  Entities: {
    main: "#d97706",
    glow: "rgba(217, 119, 6, 0.35)",
    text: "#d97706",
    desc: "Projects, codebase modules, tools, software stack, and external references.",
  },
  Constraints: {
    main: "#e11d48",
    glow: "rgba(225, 29, 72, 0.35)",
    text: "#e11d48",
    desc: "Hard system constraints, hardware limits, security bounds, and forbidden rules.",
  },
  Inactive: {
    main: "#475569",
    glow: "rgba(71, 85, 105, 0.25)",
    text: "#475569",
    desc: "Historical tombstones and superseded memory facts.",
  },
};

export function getThemeCollectionColors(isLight: boolean) {
  return isLight ? LIGHT_COLLECTION_COLORS : DARK_COLLECTION_COLORS;
}

export function getCollectionColor(rawCollection: string, isSuperseded = false, isLight = false) {
  const palette = getThemeCollectionColors(isLight);
  if (isSuperseded) return palette.Inactive;
  const norm = rawCollection.toLowerCase();
  if (norm.includes("identity")) return palette.Identity;
  if (norm.includes("profile")) return palette.Profile;
  if (norm.includes("directive")) return palette.Directives;
  if (norm.includes("narrative") || norm.includes("context")) return palette.Narrative;
  if (norm.includes("entity") || norm.includes("entities") || norm.includes("project")) return palette.Entities;
  if (norm.includes("constraint")) return palette.Constraints;
  return palette.Identity;
}

export function getRelationStyle(rawRelation: string, isLight = false) {
  const norm = rawRelation.toUpperCase();
  if (norm.includes("SUPPORT")) return { color: isLight ? "#059669" : "#34d399", isDashed: false };
  if (norm.includes("SUPERSEDE")) return { color: isLight ? "#0284c7" : "#38bdf8", isDashed: false };
  if (norm.includes("SHAPE")) return { color: isLight ? "#7c3aed" : "#a78bfa", isDashed: false };
  if (norm.includes("DEPEND")) return { color: isLight ? "#d97706" : "#facc15", isDashed: false };
  if (norm.includes("CONFLICT") || norm.includes("RESTRICT")) return { color: isLight ? "#dc2626" : "#ef4444", isDashed: true };
  return { color: isLight ? "#64748b" : "#64748b", isDashed: true };
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

interface CrossRelation {
  targetCollection: string;
  relation: string;
  count: number;
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
  crossRelations: CrossRelation[];
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
    const canvasContainerRef = useRef<HTMLDivElement>(null);
    const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
    const sceneRef = useRef<THREE.Scene | null>(null);
    const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
    const controlsRef = useRef<OrbitControls | null>(null);

    const instancedMeshRef = useRef<THREE.InstancedMesh | null>(null);
    const instancedRingRef = useRef<THREE.InstancedMesh | null>(null);
    const lineSegmentsRef = useRef<THREE.LineSegments | null>(null);

    const [clusterBadges, setClusterBadges] = useState<ClusterBadgeData[]>([]);
    const [expandedBadge, setExpandedBadge] = useState<string | null>(null);
    const [isLayoutStable, setIsLayoutStable] = useState(false);
    const [isLightMode, setIsLightMode] = useState(false);

    // Props Refs to avoid Three.js Scene Setup useEffect re-creation on node clicks/filters
    const selectedFactIdRef = useRef(selectedFactId);
    selectedFactIdRef.current = selectedFactId;

    const selectedFactDetailRef = useRef(selectedFactDetail);
    selectedFactDetailRef.current = selectedFactDetail;

    const searchQueryRef = useRef(searchQuery);
    searchQueryRef.current = searchQuery;

    const selectedCollectionRef = useRef(selectedCollection);
    selectedCollectionRef.current = selectedCollection;

    const selectedRelationRef = useRef(selectedRelation);
    selectedRelationRef.current = selectedRelation;

    const gNodesRef = useRef<GNode[]>([]);
    const gLinksRef = useRef<GLink[]>([]);
    const idToNodeIndexMap = useRef<Map<string, number>>(new Map());

    const hasFittedInitialCameraRef = useRef(false);
    const userHasNavigatedCameraRef = useRef(false);
    const tempVecRef = useRef(new THREE.Vector3());
    const animFrameRef = useRef<number | null>(null);

    // Detect Theme Changes
    useEffect(() => {
      const checkTheme = () => {
        const theme = document.documentElement.getAttribute("data-theme");
        setIsLightMode(theme === "light");
      };
      checkTheme();

      const observer = new MutationObserver(checkTheme);
      observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
      return () => observer.disconnect();
    }, []);

    const conflictNodeIds = useMemo(() => {
      const set = new Set<string>();
      conflictPairs.forEach((pair) => {
        set.add(pair.fact_a.id);
        set.add(pair.fact_b.id);
      });
      return set;
    }, [conflictPairs]);

    const isNodeVisible = useCallback((node: GNode) => {
      const selRel = selectedRelationRef.current;
      const selCol = selectedCollectionRef.current;

      if (selRel !== "all") {
        const hasRel = edges.some(
          (e) =>
            (e.from_id === node.id || e.to_id === node.id) &&
            e.relation.toUpperCase().includes(selRel.toUpperCase())
        );
        if (!hasRel) return false;
      }

      if (selCol === "all") return true;
      if (selCol === "Inactive") return node.status === "inactive";
      return node.collection.toLowerCase().includes(selCol.toLowerCase());
    }, [edges]);

    const isNodeMatchingSearch = useCallback((node: GNode) => {
      const sq = searchQueryRef.current.trim().toLowerCase();
      if (!sq) return true;
      const selFactDetail = selectedFactDetailRef.current;
      return (
        node.id.toLowerCase().includes(sq) ||
        node.collection.toLowerCase().includes(sq) ||
        (node.topologyNode?.fact && node.topologyNode.fact.toLowerCase().includes(sq)) ||
        (selFactDetail?.id === node.id &&
          (selFactDetail?.fact.toLowerCase().includes(sq) ||
            selFactDetail?.session_id.toLowerCase().includes(sq)))
      );
    }, []);

    // Fast 3D/2.5D Force Layout Simulation step
    const stepSimulation = useCallback(() => {
      const gNodes = gNodesRef.current;
      const gLinks = gLinksRef.current;
      if (gNodes.length === 0) return false;

      let maxVel = 0;
      const alpha = 0.08;
      const repulsion = 1200;
      const springLength = 85;

      // Node Repulsion
      for (let i = 0; i < gNodes.length; i++) {
        const na = gNodes[i];
        for (let j = i + 1; j < gNodes.length; j++) {
          const nb = gNodes[j];
          let dx = nb.x - na.x;
          let dy = nb.y - na.y;
          let dz = nb.z - na.z;
          let distSq = dx * dx + dy * dy + dz * dz + 0.1;
          if (distSq < 120000) {
            let dist = Math.sqrt(distSq);
            let force = (repulsion / (distSq * dist)) * alpha;
            let fx = dx * force;
            let fy = dy * force;
            let fz = dz * force;

            na.vx -= fx;
            na.vy -= fy;
            na.vz -= fz;
            nb.vx += fx;
            nb.vy += fy;
            nb.vz += fz;
          }
        }
      }

      // Link Springs
      for (let k = 0; k < gLinks.length; k++) {
        const link = gLinks[k];
        const na = gNodes[link.sourceIndex];
        const nb = gNodes[link.targetIndex];
        if (!na || !nb) continue;

        let dx = nb.x - na.x;
        let dy = nb.y - na.y;
        let dz = nb.z - na.z;
        let dist = Math.sqrt(dx * dx + dy * dy + dz * dz) + 0.01;
        let delta = dist - springLength;
        let force = (delta / dist) * alpha * 0.3;

        let fx = dx * force;
        let fy = dy * force;
        let fz = dz * force;

        na.vx += fx;
        na.vy += fy;
        na.vz += fz;
        nb.vx -= fx;
        nb.vy -= fy;
        nb.vz -= fz;
      }

      // Apply velocity and damping
      for (let i = 0; i < gNodes.length; i++) {
        const n = gNodes[i];
        n.vx *= 0.85;
        n.vy *= 0.85;
        n.vz *= 0.85;

        n.x += n.vx;
        n.y += n.vy;
        n.z += n.vz;

        let vel = Math.abs(n.vx) + Math.abs(n.vy) + Math.abs(n.vz);
        if (vel > maxVel) maxVel = vel;

        nodePosCache.set(n.id, { x: n.x, y: n.y, z: n.z });
      }

      return maxVel > 0.15;
    }, []);

    // Helper: Compute Bounding Sphere Camera Distance for 100% Zoomed-Out View
    const fitCameraToEntireGraph = useCallback(() => {
      const camera = cameraRef.current;
      const controls = controlsRef.current;
      const gNodes = gNodesRef.current;
      if (!camera || !controls || gNodes.length === 0) return;

      let maxRadiusSq = 0;
      gNodes.forEach((n) => {
        const distSq = n.x * n.x + n.y * n.y + n.z * n.z;
        if (distSq > maxRadiusSq) maxRadiusSq = distSq;
      });

      const radius = Math.sqrt(maxRadiusSq);
      // At FOV = 60°, Z distance = radius / sin(FOV/2) * padding = radius * 2.5 + padding
      const targetZ = Math.max(2800, radius * 3.2);

      controls.target.set(0, 0, 0);
      camera.position.set(0, 0, targetZ);
      controls.update();
      hasFittedInitialCameraRef.current = true;
    }, []);

    // Build GNodes and GLinks from backend topology & Synchronously Pre-warm Simulation
    useEffect(() => {
      if (nodes.length === 0) {
        gNodesRef.current = [];
        gLinksRef.current = [];
        idToNodeIndexMap.current.clear();
        setIsLayoutStable(true);
        return;
      }

      if (gNodesRef.current.length === 0) {
        setIsLayoutStable(false);
      }

      const degreeMap = new Map<string, number>();
      nodes.forEach((n) => degreeMap.set(n.id, 0));
      edges.forEach((e) => {
        degreeMap.set(e.from_id, (degreeMap.get(e.from_id) || 0) + 1);
        degreeMap.set(e.to_id, (degreeMap.get(e.to_id) || 0) + 1);
      });

      const nodeIndexMap = new Map<string, number>();
      const gNodes: GNode[] = nodes.map((n, idx) => {
        nodeIndexMap.set(n.id, idx);
        const colPalette = getCollectionColor(n.collection, n.is_superseded, isLightMode);
        const compactId = n.id.startsWith("mem_")
          ? `MEM-${n.id.split("_")[1]?.slice(0, 6) || n.id.slice(4, 10)}`
          : n.id;

        const cachedPos = nodePosCache.get(n.id);
        let x: number, y: number, z: number;
        if (cachedPos) {
          x = cachedPos.x;
          y = cachedPos.y;
          z = cachedPos.z;
        } else {
          const angle = (idx / Math.max(1, nodes.length)) * Math.PI * 2;
          const dist = 80 + (idx % 15) * 45;
          x = Math.cos(angle) * dist;
          y = Math.sin(angle) * dist;
          z = (Math.random() - 0.5) * 10;
          nodePosCache.set(n.id, { x, y, z });
        }

        return {
          id: n.id,
          label: n.id,
          compactId,
          collection: n.collection,
          status: n.is_superseded ? "inactive" : "active",
          topologyNode: n,
          color: colPalette.main,
          degree: degreeMap.get(n.id) || 0,
          x,
          y,
          z,
          vx: 0,
          vy: 0,
          vz: 0,
        };
      });

      idToNodeIndexMap.current = nodeIndexMap;

      const gLinks: GLink[] = [];
      edges.forEach((rel) => {
        const srcIdx = nodeIndexMap.get(rel.from_id);
        const tgtIdx = nodeIndexMap.get(rel.to_id);
        if (srcIdx !== undefined && tgtIdx !== undefined) {
          const relStyle = getRelationStyle(rel.relation, isLightMode);
          gLinks.push({
            id: `rel_${rel.id}`,
            sourceIndex: srcIdx,
            targetIndex: tgtIdx,
            fromId: rel.from_id,
            toId: rel.to_id,
            relation: rel.relation,
            color: relStyle.color,
            isDashed: relStyle.isDashed,
          });
        }
      });

      gNodesRef.current = gNodes;
      gLinksRef.current = gLinks;

      // Synchronous Pre-warm (40 ticks) before first canvas render
      for (let i = 0; i < 40; i++) {
        stepSimulation();
      }

      // Initial Camera Fit: Only set zoomed-out position if user hasn't manually zoomed/panned
      if (!userHasNavigatedCameraRef.current && cameraRef.current && controlsRef.current) {
        fitCameraToEntireGraph();
      }
    }, [nodes, edges, isLightMode, stepSimulation, fitCameraToEntireGraph]);

    // 100% Pixel-Perfect Centroid Badges Update per Animation Frame
    const updateCentroidBadgesSync = useCallback(() => {
      const camera = cameraRef.current;
      const renderer = rendererRef.current;
      const gNodes = gNodesRef.current;
      if (!camera || !renderer || gNodes.length === 0) return;

      const groups = new Map<string, { nodes: GNode[]; color: string }>();

      gNodes.forEach((n) => {
        if (!isNodeVisible(n)) return;
        const col = n.collection || "Identity";
        if (!groups.has(col)) {
          groups.set(col, { nodes: [], color: n.color });
        }
        groups.get(col)!.nodes.push(n);
      });

      const badges: ClusterBadgeData[] = [];
      const tempVec = tempVecRef.current;
      const paletteMap = getThemeCollectionColors(isLightMode);

      groups.forEach((data, colName) => {
        if (data.nodes.length === 0) return;

        let sumX = 0, sumY = 0, sumZ = 0;
        data.nodes.forEach((n) => {
          sumX += n.x;
          sumY += n.y;
          sumZ += n.z;
        });

        const gx = sumX / data.nodes.length;
        const gy = sumY / data.nodes.length;
        const gz = sumZ / data.nodes.length;

        tempVec.set(gx, gy, gz);
        tempVec.project(camera);

        const screenX = ((tempVec.x + 1) * width) / 2;
        const screenY = ((-tempVec.y + 1) * height) / 2;

        if (isNaN(screenX) || isNaN(screenY)) return;

        const palette = paletteMap[colName] || paletteMap.Identity;
        const colNodesSet = new Set(data.nodes.map((n) => n.id));

        // Calculate unique cross-collection relation edge counts
        const relMap = new Map<string, number>();
        edges.forEach((e) => {
          const fromInCol = colNodesSet.has(e.from_id);
          const toInCol = colNodesSet.has(e.to_id);

          if (fromInCol && !toInCol) {
            const targetNode = gNodes.find((n) => n.id === e.to_id);
            if (targetNode) {
              const key = `${e.relation.toUpperCase()} ➔ ${targetNode.collection}`;
              relMap.set(key, (relMap.get(key) || 0) + 1);
            }
          } else if (!fromInCol && toInCol) {
            const srcNode = gNodes.find((n) => n.id === e.from_id);
            if (srcNode) {
              const key = `${srcNode.collection} ➔ ${e.relation.toUpperCase()}`;
              relMap.set(key, (relMap.get(key) || 0) + 1);
            }
          }
        });

        const crossRelations: CrossRelation[] = Array.from(relMap.entries())
          .map(([key, count]) => {
            const parts = key.split(" ➔ ");
            return {
              relation: parts[0] || key,
              targetCollection: parts[1] || "",
              count,
            };
          })
          .sort((a, b) => b.count - a.count)
          .slice(0, 4);

        const totalRels = edges.filter((e) =>
          data.nodes.some((n) => n.id === e.from_id || n.id === e.to_id)
        ).length;
        const avgConn = data.nodes.length > 0 ? (totalRels / data.nodes.length).toFixed(2) : "0.00";

        badges.push({
          collection: colName,
          graphX: gx,
          graphY: gy,
          screenX,
          screenY,
          factCount: data.nodes.length,
          color: palette.main,
          desc: palette.desc,
          activeFacts: data.nodes.filter((n) => n.status === "active").length,
          totalRelations: totalRels,
          avgConnections: Number(avgConn),
          crossRelations,
        });
      });

      setClusterBadges(badges);
    }, [edges, width, height, isLightMode, isNodeVisible]);

    // Recenter Camera imperatively
    useImperativeHandle(ref, () => ({
      recenter: () => {
        userHasNavigatedCameraRef.current = false;
        fitCameraToEntireGraph();
      },
    }));

    // Update WebGL InstancedMesh and LineBuffers from GNodes & GLinks
    const updateWebGLBuffers = useCallback(() => {
      const gNodes = gNodesRef.current;
      const gLinks = gLinksRef.current;

      const instancedMesh = instancedMeshRef.current;
      const instancedRing = instancedRingRef.current;
      const lineSegments = lineSegmentsRef.current;

      if (!instancedMesh || !instancedRing || !lineSegments) return;

      const dummy = new THREE.Object3D();
      const color = new THREE.Color();

      // Update Node Instances
      let visibleNodeCount = 0;
      const selFactId = selectedFactIdRef.current;
      const sq = searchQueryRef.current.trim().toLowerCase();
      const hasSearch = sq.length > 0;

      gNodes.forEach((node, i) => {
        const visible = isNodeVisible(node);
        const matchesSearch = !hasSearch || isNodeMatchingSearch(node);
        const colPalette = getCollectionColor(node.collection, node.status === "inactive", isLightMode);
        const isSelected = selFactId === node.id;
        const isConflict = conflictNodeIds.has(node.id);

        let radius: number;
        let mainColorHex: string;

        if (!visible) {
          radius = 0;
          mainColorHex = colPalette.main;
        } else if (hasSearch && !matchesSearch) {
          // Ghosted Context Node
          radius = 2.0;
          mainColorHex = isLightMode ? "#94a3b8" : "#334155";
        } else {
          // Active Matching Node
          radius = isSelected
            ? 12
            : node.status === "inactive"
            ? 3.5
            : Math.min(10, 4.5 + node.degree * 1.2);

          mainColorHex = isConflict
            ? isLightMode
              ? "#dc2626"
              : "#ef4444"
            : isSelected
            ? isLightMode
              ? "#0284c7"
              : "#38bdf8"
            : colPalette.main;
        }

        dummy.position.set(node.x, node.y, node.z);
        dummy.scale.set(radius, radius, radius);
        dummy.updateMatrix();

        instancedMesh.setMatrixAt(i, dummy.matrix);
        color.set(mainColorHex);
        instancedMesh.setColorAt(i, color);

        // Ring Halos for high degree, selected, or active search matches
        if (visible && matchesSearch && (node.degree >= 2 || isSelected || isConflict || hasSearch)) {
          dummy.scale.set(radius + 2.5, radius + 2.5, 1);
          dummy.updateMatrix();
          instancedRing.setMatrixAt(visibleNodeCount, dummy.matrix);
          instancedRing.setColorAt(visibleNodeCount, color);
          visibleNodeCount++;
        }
      });

      instancedMesh.count = gNodes.length;
      instancedMesh.instanceMatrix.needsUpdate = true;
      if (instancedMesh.instanceColor) instancedMesh.instanceColor.needsUpdate = true;

      instancedRing.count = visibleNodeCount;
      instancedRing.instanceMatrix.needsUpdate = true;
      if (instancedRing.instanceColor) instancedRing.instanceColor.needsUpdate = true;

      // Update Line Buffer Attributes
      const lineGeo = lineSegments.geometry;
      const posAttr = lineGeo.attributes.position as THREE.BufferAttribute;
      const colAttr = lineGeo.attributes.color as THREE.BufferAttribute;

      const posArray = posAttr.array as Float32Array;
      const colArray = colAttr.array as Float32Array;

      let posOffset = 0;
      let activeLinks = 0;
      gLinks.forEach((link) => {
        const na = gNodes[link.sourceIndex];
        const nb = gNodes[link.targetIndex];
        if (!na || !nb || !isNodeVisible(na) || !isNodeVisible(nb)) return;

        const naMatch = !hasSearch || isNodeMatchingSearch(na);
        const nbMatch = !hasSearch || isNodeMatchingSearch(nb);
        const bothMatch = naMatch && nbMatch;

        posArray[posOffset] = na.x;
        posArray[posOffset + 1] = na.y;
        posArray[posOffset + 2] = na.z;

        posArray[posOffset + 3] = nb.x;
        posArray[posOffset + 4] = nb.y;
        posArray[posOffset + 5] = nb.z;

        if (hasSearch && !bothMatch) {
          // Ghosted Edge connecting non-matching nodes
          color.set(isLightMode ? "#e2e8f0" : "#1e293b");
        } else {
          // Active Edge connecting matching nodes
          color.set(link.color || (isLightMode ? "#94a3b8" : "#475569"));
        }

        colArray[posOffset] = color.r;
        colArray[posOffset + 1] = color.g;
        colArray[posOffset + 2] = color.b;

        colArray[posOffset + 3] = color.r;
        colArray[posOffset + 4] = color.g;
        colArray[posOffset + 5] = color.b;

        posOffset += 6;
        activeLinks++;
      });

      lineGeo.setDrawRange(0, activeLinks * 2);
      posAttr.needsUpdate = true;
      colAttr.needsUpdate = true;
    }, [conflictNodeIds, isLightMode, isNodeVisible, isNodeMatchingSearch]);

    const updateWebGLBuffersRef = useRef(updateWebGLBuffers);
    updateWebGLBuffersRef.current = updateWebGLBuffers;

    const updateCentroidBadgesSyncRef = useRef(updateCentroidBadgesSync);
    updateCentroidBadgesSyncRef.current = updateCentroidBadgesSync;

    // Imperatively trigger GPU buffer updates when prop refs change without re-creating Three.js scene
    useEffect(() => {
      updateWebGLBuffers();
    }, [selectedFactId, selectedFactDetail, searchQuery, selectedCollection, selectedRelation, updateWebGLBuffers]);

    const flyToTargetRef = useRef<{ x: number; y: number; z: number } | null>(null);

    // Trigger smooth camera fly-to when selectedFactId changes and is non-null
    useEffect(() => {
      if (!selectedFactId) {
        flyToTargetRef.current = null;
        return;
      }
      const targetNode = gNodesRef.current.find((n) => n.id === selectedFactId);
      if (targetNode) {
        const currentZ = cameraRef.current ? cameraRef.current.position.z : 1200;
        const targetZ = Math.min(currentZ, 1200);
        flyToTargetRef.current = { x: targetNode.x, y: targetNode.y, z: targetZ };
      }
    }, [selectedFactId]);

    // Three.js Scene Setup & Render Loop (NEVER destroyed on layout stabilization or prop changes)
    useEffect(() => {
      const container = canvasContainerRef.current;
      if (!container || width === 0 || height === 0) return;

      // 1. Scene
      const scene = new THREE.Scene();
      sceneRef.current = scene;

      // 2. Camera initialized to zoomed-out default distance (2800)
      const camera = new THREE.PerspectiveCamera(60, width / height, 1, 30000);
      camera.position.set(0, 0, 2800);
      cameraRef.current = camera;

      // 3. Renderer
      const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
      renderer.setSize(width, height);
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      container.appendChild(renderer.domElement);
      rendererRef.current = renderer;

      // 4. OrbitControls configured explicitly for 2.5D Plane Panning (Left Drag = Pan)
      const controls = new OrbitControls(camera, renderer.domElement);
      controls.enableRotate = false; // Disable 3D rotation for pure 2D plane navigation
      controls.mouseButtons = {
        LEFT: THREE.MOUSE.PAN,
        MIDDLE: THREE.MOUSE.DOLLY,
        RIGHT: THREE.MOUSE.PAN,
      };
      controls.touches = {
        ONE: THREE.TOUCH.PAN,
        TWO: THREE.TOUCH.DOLLY_PAN,
      };
      controls.enableDamping = true;
      controls.dampingFactor = 0.08;
      controls.minDistance = 10;
      controls.maxDistance = 20000;
      controls.zoomSpeed = 0.8;
      controls.panSpeed = 0.8;
      controls.screenSpacePanning = true;
      controlsRef.current = controls;

      // Track user interaction so topology updates don't overwrite user pan/zoom
      controls.addEventListener("start", () => {
        userHasNavigatedCameraRef.current = true;
      });
      controls.addEventListener("change", () => {
        userHasNavigatedCameraRef.current = true;
      });

      // 5. InstancedMesh for Nodes (Single GPU Draw Call for ALL Nodes)
      const maxNodes = 10000;
      const sphereGeo = new THREE.SphereGeometry(1, 14, 14);
      const nodeMat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.9 });
      const instancedMesh = new THREE.InstancedMesh(sphereGeo, nodeMat, maxNodes);
      instancedMesh.count = 0;
      scene.add(instancedMesh);
      instancedMeshRef.current = instancedMesh;

      // 6. InstancedMesh for Glow Halo Rings
      const ringGeo = new THREE.RingGeometry(1, 1.35, 18);
      const ringMat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.4, side: THREE.DoubleSide });
      const instancedRing = new THREE.InstancedMesh(ringGeo, ringMat, maxNodes);
      instancedRing.count = 0;
      scene.add(instancedRing);
      instancedRingRef.current = instancedRing;

      // 7. LineSegments for Edges (Single GPU Draw Call for ALL Edges)
      const maxEdges = 20000;
      const linePositions = new Float32Array(maxEdges * 6);
      const lineColors = new Float32Array(maxEdges * 6);

      const lineGeo = new THREE.BufferGeometry();
      lineGeo.setAttribute("position", new THREE.BufferAttribute(linePositions, 3));
      lineGeo.setAttribute("color", new THREE.BufferAttribute(lineColors, 3));

      const lineMat = new THREE.LineBasicMaterial({
        vertexColors: true,
        transparent: true,
        opacity: isLightMode ? 0.6 : 0.45,
      });

      const lineSegments = new THREE.LineSegments(lineGeo, lineMat);
      scene.add(lineSegments);
      lineSegmentsRef.current = lineSegments;

      // Animation & Simulation Loop
      let ticks = 0;
      const render = () => {
        animFrameRef.current = requestAnimationFrame(render);

        // Smooth Camera Fly-To Lerp
        if (flyToTargetRef.current && cameraRef.current && controlsRef.current) {
          const target = flyToTargetRef.current;
          const cam = cameraRef.current;
          const ctrl = controlsRef.current;

          ctrl.target.x += (target.x - ctrl.target.x) * 0.12;
          ctrl.target.y += (target.y - ctrl.target.y) * 0.12;
          ctrl.target.z += (0 - ctrl.target.z) * 0.12;

          cam.position.x += (target.x - cam.position.x) * 0.12;
          cam.position.y += (target.y - cam.position.y) * 0.12;
          cam.position.z += (target.z - cam.position.z) * 0.12;

          const dx = target.x - cam.position.x;
          const dy = target.y - cam.position.y;
          const dz = target.z - cam.position.z;
          if (dx * dx + dy * dy + dz * dz < 2) {
            flyToTargetRef.current = null;
          }
        }

        controls.update();

        const isMoving = stepSimulation();
        ticks++;

        updateWebGLBuffersRef.current();
        updateCentroidBadgesSyncRef.current();

        // Guaranteed stabilization after 35 ticks (~0.6s)
        if (ticks >= 35 || !isMoving) {
          setIsLayoutStable(true);
        }

        renderer.render(scene, camera);
      };

      render();

      return () => {
        if (animFrameRef.current) cancelAnimationFrame(animFrameRef.current);
        controls.dispose();
        renderer.dispose();
        if (container && renderer.domElement) {
          container.removeChild(renderer.domElement);
        }
      };
    }, [width, height, isLightMode, stepSimulation]);

    // Dismiss expanded cluster badge card on click outside
    useEffect(() => {
      if (!expandedBadge) return;
      const handleDocClick = (e: MouseEvent | PointerEvent) => {
        const badgeElem = document.getElementById(`badge-card-${expandedBadge}`);
        if (badgeElem && !badgeElem.contains(e.target as Node)) {
          setExpandedBadge(null);
        }
      };
      const timer = setTimeout(() => {
        document.addEventListener("pointerdown", handleDocClick);
      }, 50);
      return () => {
        clearTimeout(timer);
        document.removeEventListener("pointerdown", handleDocClick);
      };
    }, [expandedBadge]);

    // Handle Raycaster + Screen-Space Proximity Node Click Picking
    const handlePointerDown = useCallback(
      (e: React.PointerEvent<HTMLDivElement>) => {
        // If clicking on floating UI controls or overlay buttons, let their click handlers handle it
        if ((e.target as HTMLElement).closest(".pointer-events-auto") && (e.target as HTMLElement) !== canvasContainerRef.current) {
          return;
        }

        const renderer = rendererRef.current;
        const camera = cameraRef.current;
        const instancedMesh = instancedMeshRef.current;
        const gNodes = gNodesRef.current;

        if (!renderer || !camera || !instancedMesh || gNodes.length === 0) return;

        const rect = renderer.domElement.getBoundingClientRect();
        const clickX = e.clientX - rect.left;
        const clickY = e.clientY - rect.top;

        // 1. Raycaster Direct Hit Test
        const mouse = new THREE.Vector2(
          (clickX / rect.width) * 2 - 1,
          -(clickY / rect.height) * 2 + 1
        );

        const raycaster = new THREE.Raycaster();
        raycaster.setFromCamera(mouse, camera);

        const intersects = raycaster.intersectObject(instancedMesh);
        if (intersects.length > 0 && intersects[0].instanceId !== undefined) {
          const idx = intersects[0].instanceId;
          const node = gNodes[idx];
          if (node && isNodeVisible(node)) {
            onSelectNode(node.id, { x: e.clientX, y: e.clientY });
            return;
          }
        }

        // 2. Fallback Screen-Space Proximity Hit Test (24px Touch/Click Radius Threshold)
        let closestNode: GNode | null = null;
        let minSqDist = 24 * 24; // 24px radius
        const tempVec = tempVecRef.current;

        for (let i = 0; i < gNodes.length; i++) {
          const n = gNodes[i];
          if (!isNodeVisible(n)) continue;

          tempVec.set(n.x, n.y, n.z);
          tempVec.project(camera);

          const screenX = ((tempVec.x + 1) * width) / 2;
          const screenY = ((-tempVec.y + 1) * height) / 2;

          const dx = screenX - clickX;
          const dy = screenY - clickY;
          const sqDist = dx * dx + dy * dy;

          if (sqDist < minSqDist) {
            minSqDist = sqDist;
            closestNode = n;
          }
        }

        if (closestNode) {
          onSelectNode(closestNode.id, { x: e.clientX, y: e.clientY });
          return;
        }

        onSelectNode(null);
      },
      [onSelectNode, isNodeVisible, width, height]
    );

    return (
      <GraphErrorBoundary>
        <div
          ref={canvasContainerRef}
          onPointerDown={handlePointerDown}
          className="relative w-full h-full cursor-grab active:cursor-grabbing select-none"
        >
          {/* Borderless Organic Memory Network Loader Overlay */}
          <AnimatePresence>
            {!isLayoutStable && (
              <motion.div
                initial={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.6, ease: "easeInOut" }}
                className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-[rgb(var(--background))]/90 backdrop-blur-3xl pointer-events-none select-none"
              >
                {/* Orbital Glowing Central Core */}
                <div className="relative flex items-center justify-center w-28 h-28 mb-8">
                  <div className="absolute inset-0 rounded-full bg-[rgb(var(--accent))]/10 animate-ping duration-1000" />
                  <div className="absolute inset-2 rounded-full border border-[rgb(var(--accent))]/25 animate-spin duration-[6000ms]" />
                  <div className="absolute inset-5 rounded-full border border-dashed border-[rgb(var(--accent))]/40 animate-spin duration-[10000ms] [animation-direction:reverse]" />
                  <div className="relative z-10 p-4 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] shadow-[0_0_40px_rgba(var(--accent),0.3)]">
                    <Sparkles size={30} className="animate-pulse" />
                  </div>
                </div>

                {/* Clean Borderless Typography */}
                <div className="flex flex-col items-center text-center gap-1.5">
                  <h3 className="text-[15px] font-sans font-extrabold tracking-wide text-[rgb(var(--foreground))]">
                    Building memory graph...
                  </h3>
                  <p className="text-[12px] font-sans font-medium text-[rgb(var(--foreground-muted))]">
                    {nodes.length.toLocaleString()} nodes · {edges.length.toLocaleString()} edges
                  </p>
                  <p className="text-[11px] font-sans font-semibold text-[rgb(var(--foreground-muted))]/60 tracking-wider uppercase mt-2">
                    Optimizing layout and relationships
                  </p>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* Prominent & Legible Cluster Centroid Badges */}
          {isLayoutStable && (
            <div className="absolute inset-0 pointer-events-none overflow-hidden z-10">
              {clusterBadges.map((badge) => {
                const IconComp = getCollectionIcon(badge.collection);
                const isExpanded = expandedBadge === badge.collection;

                return (
                  <div
                    key={badge.collection}
                    id={`badge-card-${badge.collection}`}
                    style={{
                      left: `${badge.screenX}px`,
                      top: `${badge.screenY}px`,
                      transform: "translate(-50%, -50%)",
                    }}
                    className="absolute pointer-events-auto z-20"
                  >
                    <motion.div
                      layout
                      transition={{ type: "spring", stiffness: 380, damping: 28 }}
                      onClick={(e) => {
                        e.stopPropagation();
                        setExpandedBadge((prev) => (prev === badge.collection ? null : badge.collection));
                      }}
                      style={{
                        borderColor: isExpanded ? `${badge.color}60` : `${badge.color}35`,
                        boxShadow: isExpanded
                          ? `0 16px 45px -8px ${badge.color}45, 0 0 20px ${badge.color}25`
                          : `0 4px 20px ${badge.color}20`,
                        backgroundImage: isExpanded
                          ? `radial-gradient(circle at top, ${badge.color}15, transparent 70%)`
                          : undefined,
                      }}
                      className={cn(
                        "glass-card bg-[rgb(var(--card))]/90 backdrop-blur-[24px] border transition-colors cursor-pointer select-none text-[rgb(var(--foreground))] overflow-hidden",
                        isExpanded ? "w-[310px] p-4 rounded-3xl" : "flex items-center gap-2.5 px-4 py-2 rounded-full hover:scale-105 transition-transform"
                      )}
                    >
                      {!isExpanded ? (
                        /* Compact Button View */
                        <div className="flex items-center gap-2.5 w-full">
                          <IconComp size={16} style={{ color: badge.color }} className="shrink-0" />
                          <span className="text-[12px] font-sans font-black tracking-wider text-[rgb(var(--foreground))] uppercase">
                            {badge.collection}
                          </span>
                          <span
                            className="text-[11px] font-mono font-bold px-2.5 py-0.5 rounded-full shadow-xs"
                            style={{ backgroundColor: `${badge.color}25`, color: badge.color }}
                          >
                            {badge.factCount}
                          </span>
                        </div>
                      ) : (
                        /* Expanded Badge Card View */
                        <div className="flex flex-col gap-3 w-full">
                          {/* Header with Close Button */}
                          <div className="flex items-center justify-between border-b pb-2.5" style={{ borderColor: `${badge.color}25` }}>
                            <div className="flex items-center gap-2.5">
                              <div
                                className="p-2 rounded-xl flex items-center justify-center shrink-0 shadow-xs"
                                style={{ backgroundColor: `${badge.color}20`, color: badge.color }}
                              >
                                <IconComp size={16} />
                              </div>
                              <div className="flex flex-col">
                                <span className="text-[12px] font-sans font-black tracking-wider uppercase text-[rgb(var(--foreground))]">
                                  {badge.collection}
                                </span>
                                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                                  {badge.activeFacts} Active Facts
                                </span>
                              </div>
                            </div>

                            <div className="flex items-center gap-2">
                              <span
                                className="text-[10px] font-mono font-bold px-2.5 py-1 rounded-full shadow-xs"
                                style={{ backgroundColor: `${badge.color}25`, color: badge.color }}
                              >
                                {badge.factCount} Facts
                              </span>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setExpandedBadge(null);
                                }}
                                className="p-1.5 rounded-xl hover:bg-black/10 dark:hover:bg-white/10 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                                title="Close details"
                              >
                                <X size={14} />
                              </button>
                            </div>
                          </div>

                          {/* Description */}
                          <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] leading-relaxed">
                            {badge.desc}
                          </p>

                          {/* Cross-Collection Directed Edges */}
                          {badge.crossRelations.length > 0 && (
                            <div className="flex flex-col gap-1.5 pt-2 border-t" style={{ borderColor: `${badge.color}20` }}>
                              <div className="flex items-center justify-between px-0.5">
                                <span className="text-[10px] font-mono font-bold uppercase tracking-wider" style={{ color: badge.color }}>
                                  Connected Clusters
                                </span>
                                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                                  {badge.totalRelations} Edges
                                </span>
                              </div>

                              <div className="flex flex-col gap-1.5">
                                {badge.crossRelations.map((rel, idx) => {
                                  const targetColColor = getCollectionColor(rel.targetCollection, false, isLightMode).main;
                                  return (
                                    <div
                                      key={idx}
                                      className="flex items-center justify-between text-[11px] font-sans p-2 rounded-xl bg-black/[0.04] dark:bg-white/[0.04] border border-[rgba(var(--border),0.08)]"
                                    >
                                      <div className="flex items-center gap-1.5 font-mono text-[11px] truncate">
                                        <span className="font-bold" style={{ color: badge.color }}>
                                          {rel.relation}
                                        </span>
                                        <span className="text-[rgb(var(--foreground-muted))]">➔</span>
                                        <span className="font-semibold text-[rgb(var(--foreground))] truncate" style={{ color: targetColColor }}>
                                          {rel.targetCollection}
                                        </span>
                                      </div>
                                      <span
                                        className="font-mono font-bold text-[10px] px-2 py-0.5 rounded-full shrink-0 shadow-xs"
                                        style={{ backgroundColor: `${badge.color}20`, color: badge.color }}
                                      >
                                        {rel.count}
                                      </span>
                                    </div>
                                  );
                                })}
                              </div>
                            </div>
                          )}
                        </div>
                      )}
                    </motion.div>
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
