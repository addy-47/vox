import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { MemoryNodeTopology, MemoryEdgeTopology } from "@/services/memoryService";
import {
  GNode,
  GLink,
  ClusterBadgeData,
  CrossRelation,
  getCollectionColor,
  getRelationStyle,
  getThemeCollectionColors,
} from "@/shared/components/memory/memoryGraphTypes";

// Persistent node position cache across re-renders
const nodePosCache = new Map<string, { x: number; y: number; z: number }>();

export function clearMemoryGraphPositionCache() {
  nodePosCache.clear();
}

interface UseMemoryGraphSceneOptions {
  canvasContainerRef: React.RefObject<HTMLDivElement | null>;
  nodes: MemoryNodeTopology[];
  edges: MemoryEdgeTopology[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  selectedRelation: string;
  selectedFactId: string | null;
  selectedFactDetail?: any;
  conflictPairs?: { fact_a: MemoryNodeTopology; fact_b: MemoryNodeTopology }[];
  clearCacheOnUnmount?: boolean;
}

export function useMemoryGraphScene({
  canvasContainerRef,
  nodes,
  edges,
  width,
  height,
  searchQuery,
  selectedCollection,
  selectedRelation,
  selectedFactId,
  selectedFactDetail,
  conflictPairs = [],
  clearCacheOnUnmount = false,
}: UseMemoryGraphSceneOptions) {
  const [isLayoutStable, setIsLayoutStable] = useState(false);
  const [isLightMode, setIsLightMode] = useState(false);
  const [clusterBadges, setClusterBadges] = useState<ClusterBadgeData[]>([]);

  // Three.js Core Refs
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);

  // Mesh & Geometry Refs
  const instancedMeshRef = useRef<THREE.InstancedMesh | null>(null);
  const instancedRingRef = useRef<THREE.InstancedMesh | null>(null);
  const lineSegmentsRef = useRef<THREE.LineSegments | null>(null);

  // Simulation & Graph Data Refs
  const gNodesRef = useRef<GNode[]>([]);
  const gLinksRef = useRef<GLink[]>([]);
  const idToNodeIndexMap = useRef<Map<string, number>>(new Map());
  const isSettledRef = useRef(false);
  const ticksRef = useRef(0);
  const animFrameRef = useRef<number | null>(null);
  const isRenderingRef = useRef(false);
  const wakeRenderLoopRef = useRef<() => void>(() => {});
  const userHasNavigatedCameraRef = useRef(false);
  const hasFittedInitialCameraRef = useRef(false);
  const lastBadgeUpdateRef = useRef(0);
  const graphRadiusRef = useRef<number>(800);
  const flyToTargetRef = useRef<{ x: number; y: number; z: number } | null>(null);

  // Math Scratch Objects
  const dummyObjRef = useRef(new THREE.Object3D());
  const colorObjRef = useRef(new THREE.Color());
  const tempVecRef = useRef(new THREE.Vector3());

  // Prop Mirror Refs to prevent recreating Three.js scene on filter/search updates
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

  // Track viewport aspect ratio dynamically (vw / vh)
  const aspect = useMemo(() => {
    return width > 0 && height > 0 ? width / height : 1.77;
  }, [width, height]);

  // Detect dark / light mode from documentElement class attribute
  useEffect(() => {
    const updateTheme = () => {
      setIsLightMode(document.documentElement.classList.contains("light"));
    };
    updateTheme();
    const observer = new MutationObserver(updateTheme);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  // Cache eviction on unmount or excessive size
  useEffect(() => {
    return () => {
      if (clearCacheOnUnmount || nodePosCache.size > 2000) {
        nodePosCache.clear();
      }
    };
  }, [clearCacheOnUnmount]);

  const conflictNodeIds = useMemo(() => {
    const set = new Set<string>();
    conflictPairs.forEach((pair) => {
      set.add(pair.fact_a.id);
      set.add(pair.fact_b.id);
    });
    return set;
  }, [conflictPairs]);

  // Precomputed relation adjacency map: UpperRelation -> Set of Node IDs (O(1) lookups)
  const relationAdjacencyMap = useMemo(() => {
    const map = new Map<string, Set<string>>();
    for (const e of edges) {
      const relUpper = e.relation.toUpperCase();
      let set = map.get(relUpper);
      if (!set) {
        set = new Set();
        map.set(relUpper, set);
      }
      set.add(e.from_id);
      set.add(e.to_id);
    }
    return map;
  }, [edges]);

  const isNodeVisible = useCallback(
    (node: GNode) => {
      const selRel = selectedRelationRef.current;
      const selCol = selectedCollectionRef.current;

      if (selRel !== "all") {
        const selRelUpper = selRel.toUpperCase();
        let hasRel = false;
        for (const [relKey, nodeSet] of relationAdjacencyMap.entries()) {
          if (relKey.includes(selRelUpper) && nodeSet.has(node.id)) {
            hasRel = true;
            break;
          }
        }
        if (!hasRel) return false;
      }

      if (selCol === "all") return true;
      if (selCol === "Inactive") return node.status === "inactive";
      return node.collection.toLowerCase().includes(selCol.toLowerCase());
    },
    [relationAdjacencyMap]
  );

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

  // 3D/2.5D Force Layout Simulation step with dynamic aspect-ratio horizontal freedom
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

  // Compute Bounding Sphere Camera Distance dynamically considering both width (vw) and height (vh)
  const fitCameraToEntireGraph = useCallback(() => {
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    const gNodes = gNodesRef.current;
    if (!camera || !controls || gNodes.length === 0) return;

    let maxRadiusSq = 0;
    let maxAbsX = 0;
    let maxAbsY = 0;

    gNodes.forEach((n) => {
      const distSq = n.x * n.x + n.y * n.y + n.z * n.z;
      if (distSq > maxRadiusSq) maxRadiusSq = distSq;
      if (Math.abs(n.x) > maxAbsX) maxAbsX = Math.abs(n.x);
      if (Math.abs(n.y) > maxAbsY) maxAbsY = Math.abs(n.y);
    });

    const radius = Math.max(300, Math.sqrt(maxRadiusSq));
    graphRadiusRef.current = radius;

    // Viewport-aware camera distance calculation:
    // Vertical half-FOV angle = 30° -> tan(30°) ≈ 0.57735
    // Horizontal half-angle = aspect * tan(30°)
    const currentAspect = Math.max(0.5, camera.aspect || aspect || 1.77);
    const halfFovYRad = (30 * Math.PI) / 180;
    const tanHalfY = Math.tan(halfFovYRad);
    const tanHalfX = currentAspect * tanHalfY;

    // Distance required to fill ~82% of height:
    const requiredDistanceY = maxAbsY / (0.82 * tanHalfY);
    // Distance required to fill ~86% of width:
    const requiredDistanceX = maxAbsX / (0.86 * tanHalfX);

    // Distance to fit both dimensions comfortably without collapsing into a small center disc:
    const optimalFitZ = Math.max(requiredDistanceY, requiredDistanceX, radius * 1.6);
    const maxZoomOutZ = Math.max(1200, Math.min(optimalFitZ * 1.5, 4500));
    const targetZ = Math.min(maxZoomOutZ, Math.max(800, optimalFitZ));

    controls.maxDistance = maxZoomOutZ * 1.1;
    controls.minDistance = 200;

    controls.target.set(0, 0, 0);
    camera.position.set(0, 0, targetZ);
    controls.update();
    hasFittedInitialCameraRef.current = true;
  }, [aspect]);

  // Build GNodes and GLinks from backend topology with dynamic aspect-ratio shaped cluster anchors
  useEffect(() => {
    if (nodes.length === 0) {
      gNodesRef.current = [];
      gLinksRef.current = [];
      idToNodeIndexMap.current.clear();
      isSettledRef.current = true;
      setIsLayoutStable(true);
      return;
    }

    isSettledRef.current = false;
    ticksRef.current = 0;
    setIsLayoutStable(false);

    const degreeMap = new Map<string, number>();
    nodes.forEach((n) => degreeMap.set(n.id, 0));
    edges.forEach((e) => {
      degreeMap.set(e.from_id, (degreeMap.get(e.from_id) || 0) + 1);
      degreeMap.set(e.to_id, (degreeMap.get(e.to_id) || 0) + 1);
    });

    // Semantic cluster layout anchors shaped dynamically by viewport aspect ratio (width / height)
    // On widescreen displays (e.g. 16:9, 21:9), clusters spread wider horizontally across the viewport
    const aspectFactor = Math.min(Math.max(aspect, 1.2), 2.4);

    const CLUSTER_CENTROIDS: Record<string, { x: number; y: number }> = {
      Identity: { x: 0, y: 40 },
      Profile: { x: -320 * (aspectFactor / 1.5), y: 120 },
      Directives: { x: 320 * (aspectFactor / 1.5), y: 140 },
      Narrative: { x: -260 * (aspectFactor / 1.5), y: -200 },
      Entities: { x: 280 * (aspectFactor / 1.5), y: -190 },
      Constraints: { x: 0, y: -280 },
      Inactive: { x: 420 * (aspectFactor / 1.5), y: 0 },
    };

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
        // Compute initial placement with horizontal width bias based on aspect ratio
        const centroid = CLUSTER_CENTROIDS[n.collection] || { x: 0, y: 0 };
        const angle = (idx / Math.max(1, nodes.length)) * Math.PI * 2;
        const jitterRadius = 40 + (idx % 18) * 20;

        // Scale horizontal jitter by aspect ratio so node distribution fills widescreen width
        x = centroid.x + Math.cos(angle) * jitterRadius * (aspectFactor * 0.85);
        y = centroid.y + Math.sin(angle) * jitterRadius;
        z = (Math.random() - 0.5) * 15;

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

    wakeRenderLoopRef.current();
  }, [nodes, edges, isLightMode, aspect, stepSimulation, fitCameraToEntireGraph]);

  // Centroid Badges Update (Throttled to avoid 60fps React state updates)
  const updateCentroidBadgesSync = useCallback(
    (force = false) => {
      const now = performance.now();
      if (!force && now - lastBadgeUpdateRef.current < 120) return;
      lastBadgeUpdateRef.current = now;

      const camera = cameraRef.current;
      const renderer = rendererRef.current;
      const gNodes = gNodesRef.current;
      if (!camera || !renderer || gNodes.length === 0) return;

      const groups = new Map<string, { nodes: GNode[]; color: string }>();
      const nodeById = new Map<string, GNode>();

      for (let i = 0; i < gNodes.length; i++) {
        const n = gNodes[i];
        nodeById.set(n.id, n);
        if (!isNodeVisible(n)) continue;
        const col = n.collection || "Identity";
        if (!groups.has(col)) {
          groups.set(col, { nodes: [], color: n.color });
        }
        groups.get(col)!.nodes.push(n);
      }

      const badges: ClusterBadgeData[] = [];
      const tempVec = tempVecRef.current;
      const paletteMap = getThemeCollectionColors(isLightMode);

      groups.forEach((data, colName) => {
        if (data.nodes.length === 0) return;

        let sumX = 0,
          sumY = 0,
          sumZ = 0;
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
        let totalRels = 0;
        edges.forEach((e) => {
          const fromInCol = colNodesSet.has(e.from_id);
          const toInCol = colNodesSet.has(e.to_id);

          if (fromInCol || toInCol) {
            totalRels++;
          }

          if (fromInCol && !toInCol) {
            const targetNode = nodeById.get(e.to_id);
            if (targetNode) {
              const key = `${e.relation.toUpperCase()} ➔ ${targetNode.collection}`;
              relMap.set(key, (relMap.get(key) || 0) + 1);
            }
          } else if (!fromInCol && toInCol) {
            const srcNode = nodeById.get(e.from_id);
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

        const avgConn =
          data.nodes.length > 0 ? (totalRels / data.nodes.length).toFixed(2) : "0.00";

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
    },
    [edges, width, height, isLightMode, isNodeVisible]
  );

  // Update WebGL InstancedMesh and LineBuffers from GNodes & GLinks
  const updateWebGLBuffers = useCallback(() => {
    const gNodes = gNodesRef.current;
    const gLinks = gLinksRef.current;

    const instancedMesh = instancedMeshRef.current;
    const instancedRing = instancedRingRef.current;
    const lineSegments = lineSegmentsRef.current;

    if (!instancedMesh || !instancedRing || !lineSegments) return;

    const dummy = dummyObjRef.current;
    const color = colorObjRef.current;

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
        radius = 2.0;
        mainColorHex = isLightMode ? "#94a3b8" : "#334155";
      } else {
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
            ? "#000000"
            : "#ffffff"
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
        color.set(isLightMode ? "#e2e8f0" : "#1e293b");
      } else {
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

  // Trigger GPU buffer updates when prop refs change without re-creating Three.js scene
  useEffect(() => {
    updateWebGLBuffers();
    wakeRenderLoopRef.current();
  }, [
    selectedFactId,
    selectedFactDetail,
    searchQuery,
    selectedCollection,
    selectedRelation,
    updateWebGLBuffers,
  ]);

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
      wakeRenderLoopRef.current();
    }
  }, [selectedFactId]);

  // Theme synchronization without WebGL context destruction
  useEffect(() => {
    if (lineSegmentsRef.current) {
      const mat = lineSegmentsRef.current.material as THREE.LineBasicMaterial;
      mat.opacity = isLightMode ? 0.6 : 0.45;
      mat.needsUpdate = true;
    }
    updateWebGLBuffers();
    updateCentroidBadgesSync(true);
    wakeRenderLoopRef.current();
  }, [isLightMode, updateWebGLBuffers, updateCentroidBadgesSync]);

  // Three.js Scene Setup & Render Loop
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
    controls.enableRotate = false;
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

    controls.addEventListener("start", () => {
      userHasNavigatedCameraRef.current = true;
    });
    controls.addEventListener("change", () => {
      userHasNavigatedCameraRef.current = true;
    });

    // 5. InstancedMesh for Nodes
    const maxNodes = 10000;
    const sphereGeo = new THREE.SphereGeometry(1, 14, 14);
    const nodeMat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.9 });
    const instancedMesh = new THREE.InstancedMesh(sphereGeo, nodeMat, maxNodes);
    instancedMesh.count = 0;
    instancedMesh.frustumCulled = false;
    scene.add(instancedMesh);
    instancedMeshRef.current = instancedMesh;

    // 6. InstancedMesh for Glow Halo Rings
    const ringGeo = new THREE.RingGeometry(1, 1.35, 18);
    const ringMat = new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: 0.4,
      side: THREE.DoubleSide,
    });
    const instancedRing = new THREE.InstancedMesh(ringGeo, ringMat, maxNodes);
    instancedRing.count = 0;
    instancedRing.frustumCulled = false;
    scene.add(instancedRing);
    instancedRingRef.current = instancedRing;

    // 7. LineSegments for Edges
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
    lineSegments.frustumCulled = false;
    scene.add(lineSegments);
    lineSegmentsRef.current = lineSegments;

    // Initial buffer population
    updateWebGLBuffersRef.current();
    updateCentroidBadgesSyncRef.current(true);

    // Demand-Driven Animation & Simulation Loop
    const render = () => {
      isRenderingRef.current = true;

      // Smooth Camera Fly-To Lerp
      let cameraFlying = false;
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
        } else {
          cameraFlying = true;
        }
      }

      controls.update();

      // Drag & Pan Boundary Clamping (Responsive to viewport aspect ratio)
      const R = graphRadiusRef.current || 800;
      const maxPanX = R * Math.max(1.5, (width / Math.max(height, 1)) * 1.2);
      const maxPanY = R * 1.5;

      if (Math.abs(controls.target.x) > maxPanX || Math.abs(controls.target.y) > maxPanY) {
        const clampedX = Math.max(-maxPanX, Math.min(maxPanX, controls.target.x));
        const clampedY = Math.max(-maxPanY, Math.min(maxPanY, controls.target.y));
        const diffX = clampedX - controls.target.x;
        const diffY = clampedY - controls.target.y;
        controls.target.x = clampedX;
        controls.target.y = clampedY;
        camera.position.x += diffX;
        camera.position.y += diffY;
      }

      if (!isSettledRef.current) {
        const isMoving = stepSimulation();
        ticksRef.current++;
        updateWebGLBuffersRef.current();

        if (ticksRef.current >= 75 || (!isMoving && ticksRef.current >= 35)) {
          isSettledRef.current = true;
          setIsLayoutStable(true);
          updateCentroidBadgesSyncRef.current(true);
        }
      }

      updateCentroidBadgesSyncRef.current();
      renderer.render(scene, camera);

      const isInteracting = cameraFlying || !isSettledRef.current;
      if (isInteracting) {
        animFrameRef.current = requestAnimationFrame(render);
      } else {
        isRenderingRef.current = false;
        animFrameRef.current = null;
      }
    };

    const wakeRenderLoop = () => {
      if (!isRenderingRef.current) {
        isRenderingRef.current = true;
        animFrameRef.current = requestAnimationFrame(render);
      }
    };
    wakeRenderLoopRef.current = wakeRenderLoop;

    controls.addEventListener("change", wakeRenderLoop);
    controls.addEventListener("start", wakeRenderLoop);

    wakeRenderLoop();

    const failsafeTimer = setTimeout(() => {
      isSettledRef.current = true;
      setIsLayoutStable(true);
      updateCentroidBadgesSyncRef.current(true);
    }, 700);

    return () => {
      clearTimeout(failsafeTimer);
      if (animFrameRef.current) cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = null;
      isRenderingRef.current = false;
      controls.dispose();

      // GPU Resource Teardown
      if (instancedMeshRef.current) {
        instancedMeshRef.current.geometry.dispose();
        if (Array.isArray(instancedMeshRef.current.material)) {
          instancedMeshRef.current.material.forEach((m) => m.dispose());
        } else {
          instancedMeshRef.current.material.dispose();
        }
        instancedMeshRef.current = null;
      }

      if (instancedRingRef.current) {
        instancedRingRef.current.geometry.dispose();
        if (Array.isArray(instancedRingRef.current.material)) {
          instancedRingRef.current.material.forEach((m) => m.dispose());
        } else {
          instancedRingRef.current.material.dispose();
        }
        instancedRingRef.current = null;
      }

      if (lineSegmentsRef.current) {
        lineSegmentsRef.current.geometry.dispose();
        if (Array.isArray(lineSegmentsRef.current.material)) {
          lineSegmentsRef.current.material.forEach((m) => m.dispose());
        } else {
          lineSegmentsRef.current.material.dispose();
        }
        lineSegmentsRef.current = null;
      }

      if (sceneRef.current) {
        sceneRef.current.clear();
        sceneRef.current = null;
      }

      renderer.forceContextLoss();
      renderer.dispose();
      rendererRef.current = null;
      if (container && renderer.domElement && container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement);
      }

      if (clearCacheOnUnmount) {
        nodePosCache.clear();
      }
    };
  }, [stepSimulation, clearCacheOnUnmount, width, height]);

  // Dedicated lightweight camera & renderer resize effect
  useEffect(() => {
    const renderer = rendererRef.current;
    const camera = cameraRef.current;
    if (!renderer || !camera || width === 0 || height === 0) return;

    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
    updateCentroidBadgesSyncRef.current(true);
    wakeRenderLoopRef.current();
  }, [width, height]);

  // Camera navigation helpers exposed to parent
  const recenter = useCallback(() => {
    userHasNavigatedCameraRef.current = false;
    fitCameraToEntireGraph();
    wakeRenderLoopRef.current();
  }, [fitCameraToEntireGraph]);

  const zoomIn = useCallback(() => {
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (!camera || !controls) return;
    userHasNavigatedCameraRef.current = true;

    const newZ = Math.max(controls.minDistance || 200, camera.position.z * 0.75);
    camera.position.z = newZ;
    controls.update();
    wakeRenderLoopRef.current();
  }, []);

  const zoomOut = useCallback(() => {
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (!camera || !controls) return;
    userHasNavigatedCameraRef.current = true;

    const newZ = Math.min(controls.maxDistance || 4000, camera.position.z * 1.3);
    camera.position.z = newZ;
    controls.update();
    wakeRenderLoopRef.current();
  }, []);

  return {
    isLayoutStable,
    isLightMode,
    clusterBadges,
    gNodesRef,
    cameraRef,
    rendererRef,
    instancedMeshRef,
    isNodeVisible,
    recenter,
    zoomIn,
    zoomOut,
  };
}
