import { useCallback, useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { ObservationRecord } from "@/services/memoryService";
import {
  GNode,
  GLink,
  getActiveDynamicPalette,
} from "./memoryGraphTypes";
import type { GraphWorkerOutput } from "./memoryGraphWorker";
import { isSoftwareRasterizer } from "@/shared/lib/glUtils";

interface UseMemoryGraphSceneOptions {
  canvasContainerRef: React.RefObject<HTMLDivElement | null>;
  facts: ObservationRecord[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  selectedFactId: string | null;
  selectedSessionId?: string | null;
  onCoreClick?: () => void;
  clearCacheOnUnmount?: boolean;
  /** When true, suspend the rAF render loop immediately (drawer open over graph) */
  paused?: boolean;
}

interface SessionAnchor {
  sId: string;
  count: number;
  x: number;
  y: number;
  z: number;
  color: string;
  timestamp: number;
}

interface ConduitSegment {
  p0: THREE.Vector3;
  p1: THREE.Vector3;
  col0: string;
  col1: string;
  sessionId: string;
}

interface CoreMaterialUniforms {
  [uniform: string]: THREE.IUniform;
  color1: THREE.IUniform<THREE.Color>;
  color2: THREE.IUniform<THREE.Color>;
  opacity: THREE.IUniform<number>;
}

interface CoreShaderMaterial extends THREE.ShaderMaterial {
  uniforms: CoreMaterialUniforms;
}

// Reusable scratch colors to eliminate per-frame/per-link allocations
const SCRATCH_COLOR_1 = new THREE.Color();
const SCRATCH_COLOR_2 = new THREE.Color();
const SCRATCH_START_COLOR = new THREE.Color();
const SCRATCH_RING_COLOR = new THREE.Color();
const LIGHT_BG_COLOR = new THREE.Color(0xf8fafc);

export function useMemoryGraphScene({
  canvasContainerRef,
  facts,
  width,
  height,
  searchQuery,
  selectedCollection,
  selectedFactId,
  selectedSessionId = null,
  paused = false,
}: UseMemoryGraphSceneOptions) {
  const [isLightMode, setIsLightMode] = useState(false);

  // Three.js Core Refs
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);

  // Mesh & Geometry Refs
  const instancedMeshRef = useRef<THREE.InstancedMesh | null>(null);
  const instancedRingRef = useRef<THREE.InstancedMesh | null>(null);
  const lineSegmentsRef = useRef<THREE.LineSegments | null>(null);
  const coreMeshRef = useRef<THREE.Mesh | null>(null);
  const coreWireMeshRef = useRef<THREE.Mesh | null>(null);
  const coreNucleusMeshRef = useRef<THREE.Mesh | null>(null);
  const coreInnerRingRef = useRef<THREE.Mesh | null>(null);
  const coreOuterRingRef = useRef<THREE.Mesh | null>(null);
  const wakeLoopRef = useRef<() => void>(() => {});
  const renderOnceRef = useRef<() => void>(() => {});

  // Graph Data Refs
  const gNodesRef = useRef<GNode[]>([]);
  const gLinksRef = useRef<GLink[]>([]);
  const sessionAnchorsRef = useRef<SessionAnchor[]>([]);
  const conduitsRef = useRef<ConduitSegment[]>([]);
  const animFrameRef = useRef<number | null>(null);
  const flyToTargetRef = useRef<{
    cam: { x: number; y: number; z: number };
    target: { x: number; y: number; z: number };
  } | null>(null);

  // Scratch Math Objects
  const dummyObjRef = useRef(new THREE.Object3D());
  const colorObjRef = useRef(new THREE.Color());

  // Prop Mirror Refs
  const selectedFactIdRef = useRef(selectedFactId);
  const selectedSessionIdRef = useRef(selectedSessionId);
  const searchQueryRef = useRef(searchQuery);
  const selectedCollectionRef = useRef(selectedCollection);
  const factsRef = useRef(facts);
  const isLightModeRef = useRef(isLightMode);
  const pausedRef = useRef(paused);
  const widthRef = useRef(width);
  const heightRef = useRef(height);
  const rebuildTopologyRef = useRef<((inputFacts: ObservationRecord[], isLight: boolean) => void) | null>(null);

  useEffect(() => {
    selectedFactIdRef.current = selectedFactId;
    selectedSessionIdRef.current = selectedSessionId;
    searchQueryRef.current = searchQuery;
    selectedCollectionRef.current = selectedCollection;
    factsRef.current = facts;
    isLightModeRef.current = isLightMode;
    pausedRef.current = paused;
    widthRef.current = width;
    heightRef.current = height;
  });

  // ── External pause/resume: suspends or resumes the rAF render loop ──────────
  useEffect(() => {
    if (paused) {
      if (animFrameRef.current !== null) {
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = null;
      }
    } else {
      // Wake loop only if the scene has been initialized (wakeLoopRef is non-noop)
      wakeLoopRef.current();
    }
  }, [paused]);

  // Detect dark / light mode and dynamic accent changes
  useEffect(() => {
    const updateTheme = () => {
      const isLight = document.documentElement.getAttribute("data-theme") === "light";
      setIsLightMode(isLight);
      isLightModeRef.current = isLight;

      const palette = getActiveDynamicPalette(isLight);

      if (coreMeshRef.current) {
        const mat = coreMeshRef.current.material as CoreShaderMaterial;
        mat.uniforms.color1.value.set(isLight ? "#0284c7" : "#00f0ff");
        mat.uniforms.color2.value.set(isLight ? "#9333ea" : "#c084fc");
        mat.uniforms.opacity.value = isLight ? 0.85 : 0.75;
      }
      if (coreWireMeshRef.current) {
        const mat = coreWireMeshRef.current.material as THREE.MeshBasicMaterial;
        mat.color.copy(palette.personal.threeColor);
        mat.opacity = isLight ? 0.22 : 0.35;
        mat.blending = isLight ? THREE.NormalBlending : THREE.AdditiveBlending;
        mat.needsUpdate = true;
      }
      if (coreNucleusMeshRef.current) {
        const mat = coreNucleusMeshRef.current.material as THREE.MeshBasicMaterial;
        mat.color.copy(palette.personal.threeColor);
        mat.opacity = isLight ? 0.90 : 0.80;
        mat.needsUpdate = true;
      }
      if (coreInnerRingRef.current) {
        const mat = coreInnerRingRef.current.material as THREE.MeshBasicMaterial;
        mat.color.copy(palette.personal.threeColor);
        mat.opacity = isLight ? 0.55 : 0.45;
        mat.needsUpdate = true;
      }
      if (coreOuterRingRef.current) {
        const mat = coreOuterRingRef.current.material as THREE.MeshBasicMaterial;
        mat.color.copy(palette.personal.threeColor);
        mat.opacity = isLight ? 0.40 : 0.30;
        mat.needsUpdate = true;
      }
      if (lineSegmentsRef.current) {
        const lineMat = lineSegmentsRef.current.material as THREE.LineBasicMaterial;
        lineMat.blending = THREE.NormalBlending;
        lineMat.opacity = isLight ? 0.55 : 0.28;
        lineMat.needsUpdate = true;
      }
      if (instancedMeshRef.current) {
        const nodeMat = instancedMeshRef.current.material as THREE.MeshBasicMaterial;
        nodeMat.opacity = isLight ? 0.98 : 0.92;
        nodeMat.needsUpdate = true;
      }
    };

    updateTheme();
    const observer = new MutationObserver(() => {
      updateTheme();
      wakeLoopRef.current();
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  const isNodeVisible = useCallback((node: GNode) => {
    const selCol = selectedCollectionRef.current;
    if (selCol !== "all" && node.collection !== selCol) return false;
    return true;
  }, []);

  const isNodeMatchingSearch = useCallback((node: GNode) => {
    const sq = searchQueryRef.current.trim().toLowerCase();
    if (!sq) return true;
    return (
      node.collection.toLowerCase().includes(sq) ||
      (node.factRecord?.text && node.factRecord.text.toLowerCase().includes(sq))
    );
  }, []);

  // Update WebGL InstancedMesh Buffers & Colors (Jewel Nodes)
  const updateWebGLBuffers = useCallback(() => {
    const gNodes = gNodesRef.current;
    const instancedMesh = instancedMeshRef.current;
    const instancedRing = instancedRingRef.current;
    if (!instancedMesh || !instancedRing || gNodes.length === 0) return;

    const dummy = dummyObjRef.current;
    const color = colorObjRef.current;
    const selFactId = selectedFactIdRef.current;
    const selSessionId = selectedSessionIdRef.current;
    const sq = searchQueryRef.current.trim().toLowerCase();
    const hasSearch = sq.length > 0;
    const isLight = isLightModeRef.current;
    const dynamicPalette = getActiveDynamicPalette(isLight);
    const categoryColorMap: Record<string, string> = {
      personal: dynamicPalette.personal.main,
      objective: dynamicPalette.objective.main,
      workdone: dynamicPalette.workdone.main,
      blocker: dynamicPalette.blocker.main,
      next_step: dynamicPalette.next_step.main,
      pitfall: dynamicPalette.pitfall.main,
    };

    gNodes.forEach((node, i) => {
      const visible = isNodeVisible(node);
      const matchesSearch = !hasSearch || isNodeMatchingSearch(node);
      const isSelected = selFactId === node.id;
      const nodeSessionId = node.sessionId ?? (node.factRecord.session_id !== null ? String(node.factRecord.session_id) : null);
      const isSessionSelected = selSessionId !== null && nodeSessionId === selSessionId;
      const isOtherSession = selSessionId !== null && nodeSessionId !== selSessionId && node.collection !== "personal";

      const coreAccentHex = isLight ? "#0891b2" : "#00dbe9";
      const isAnchor = Boolean(node.isSessionAnchor || node.id.startsWith("anchor_"));
      const colHexMain = isAnchor
        ? coreAccentHex
        : (categoryColorMap[node.collection] ?? dynamicPalette.objective.main);

      let radius: number;
      let colHex: string;

      if (!visible) {
        radius = 0.001;
        colHex = colHexMain;
      } else if (isSelected) {
        // Selected node: refined enlargement
        radius = isAnchor ? 9.5 : node.collection === "personal" ? 7.6 : 5.6;
        colHex = colHexMain;
      } else if (hasSearch && !matchesSearch) {
        radius = 1.2;
        colHex = isLight ? "#94a3b8" : "#283344";
      } else if (isOtherSession) {
        // Non-selected session facts drop to ultra-subtle starlight dots
        radius = isAnchor ? 1.0 : 0.5;
        colHex = isLight ? "#cbd5e1" : "#1e293b";
      } else if (isSessionSelected) {
        // Vividly highlighted session nodes
        radius = isAnchor ? 8.8 : node.collection === "personal" ? 6.5 : 5.4;
        colHex = colHexMain;
      } else {
        // Normal state
        radius = isAnchor ? 7.2 : node.collection === "personal" ? 5.5 : 3.8;
        colHex = colHexMain;
      }

      // Position Node
      dummy.position.set(node.x, node.y, node.z);
      dummy.scale.set(radius, radius, radius);
      dummy.updateMatrix();
      instancedMesh.setMatrixAt(i, dummy.matrix);

      color.set(colHex);
      instancedMesh.setColorAt(i, color);

      // Ring Halo Matrix (active for selected facts, highlighted personal nodes, or active session anchor)
      if (isSelected || (isSessionSelected && (node.collection === "personal" || isAnchor))) {
        const ringScale = isSelected ? radius * 1.55 : radius * 1.35;
        dummy.position.set(node.x, node.y, node.z);
        dummy.scale.set(ringScale, ringScale, ringScale);
        dummy.lookAt(cameraRef.current ? cameraRef.current.position : new THREE.Vector3(0, 0, 3000));
        dummy.updateMatrix();
        instancedRing.setMatrixAt(i, dummy.matrix);

        SCRATCH_RING_COLOR.set(colHexMain);
        instancedRing.setColorAt(i, SCRATCH_RING_COLOR);
      } else {
        dummy.position.set(node.x, node.y, node.z);
        dummy.scale.set(0.001, 0.001, 0.001);
        dummy.updateMatrix();
        instancedRing.setMatrixAt(i, dummy.matrix);
      }
    });

    instancedMesh.count = gNodes.length;
    instancedRing.count = gNodes.length;
    if (instancedMesh.instanceMatrix) instancedMesh.instanceMatrix.needsUpdate = true;
    if (instancedMesh.instanceColor) instancedMesh.instanceColor.needsUpdate = true;
    if (instancedRing.instanceMatrix) instancedRing.instanceMatrix.needsUpdate = true;
    if (instancedRing.instanceColor) instancedRing.instanceColor.needsUpdate = true;
  }, [isNodeMatchingSearch, isNodeVisible]);

  // Update Line Conduits Highlighting & Colors
  const updateLineHighlighting = useCallback(() => {
    const lineSegments = lineSegmentsRef.current;
    const conduits = conduitsRef.current;
    const gLinks = gLinksRef.current;
    const gNodes = gNodesRef.current;
    const sessionAnchors = sessionAnchorsRef.current;
    if (!lineSegments || (conduits.length === 0 && gLinks.length === 0)) return;

    const selSessionId = selectedSessionIdRef.current;
    const selFactId = selectedFactIdRef.current;
    const selCollection = selectedCollectionRef.current;
    const sq = searchQueryRef.current.trim().toLowerCase();
    const hasSearch = sq.length > 0;
    const hasCollectionFilter = selCollection !== "all";
    const isLight = isLightModeRef.current;

    const selNode = selFactId ? gNodes.find((n) => n.id === selFactId) : null;
    const selNodeSessionId = selNode?.sessionId ?? null;

    const totalLines = conduits.length + gLinks.length;
    const requiredFloats = totalLines * 6;
    const geo = lineSegments.geometry as THREE.BufferGeometry;
    let posAttr = geo.getAttribute("position") as THREE.BufferAttribute;
    let colAttr = geo.getAttribute("color") as THREE.BufferAttribute;

    // Only reallocate if line capacity genuinely exceeds preallocated limit (40,000 lines = 240,000 floats)
    if (!posAttr || posAttr.array.length < requiredFloats) {
      posAttr = new THREE.BufferAttribute(new Float32Array(Math.max(requiredFloats, 240000)), 3);
      colAttr = new THREE.BufferAttribute(new Float32Array(Math.max(requiredFloats, 240000)), 3);
      geo.setAttribute("position", posAttr);
      geo.setAttribute("color", colAttr);
    }

    const posArray = posAttr.array as Float32Array;
    const colArray = colAttr.array as Float32Array;
    let writePtr = 0;

    // Build O(1) session anchor lookup map
    const anchorMap = new Map<string, SessionAnchor>();
    for (let i = 0; i < sessionAnchors.length; i++) {
      anchorMap.set(sessionAnchors[i].sId, sessionAnchors[i]);
    }

    /**
     * Determines if a node passes the current active filters.
     * A node is "live" if:
     *  1. Its collection matches the selected collection (or all is selected)
     *  2. It matches the search query (or no search is active)
     * Personal / core identity nodes are always live so CORE_IDENTITY links remain.
     */
    const isNodeLive = (node: GNode): boolean => {
      if (node.collection === "personal") return true; // core identity always visible
      if (hasCollectionFilter && node.collection !== selCollection) return false;
      if (hasSearch) {
        const matchesText = node.factRecord?.text?.toLowerCase().includes(sq) ?? false;
        const matchesCat = node.collection.toLowerCase().includes(sq);
        if (!matchesText && !matchesCat) return false;
      }
      return true;
    };

    // 1. Write Umbilical Conduits
    // Conduits connect session anchors to the core — skip if collection-filtered away
    conduits.forEach((cond) => {
      // If a collection filter is active, only show conduits for the selected session's anchor
      if (hasCollectionFilter && selSessionId === null) return;

      // Invariant: When a session is selected, all other session conduits VANISH completely!
      if (selSessionId !== null && cond.sessionId !== selSessionId) return;

      posArray[writePtr + 0] = cond.p0.x;
      posArray[writePtr + 1] = cond.p0.y;
      posArray[writePtr + 2] = cond.p0.z;
      posArray[writePtr + 3] = cond.p1.x;
      posArray[writePtr + 4] = cond.p1.y;
      posArray[writePtr + 5] = cond.p1.z;

      const isFactSession = selNodeSessionId !== null && cond.sessionId === selNodeSessionId;
      const isHighlight = (selSessionId !== null && cond.sessionId === selSessionId) || isFactSession;

      SCRATCH_COLOR_1.set(cond.col0);
      SCRATCH_COLOR_2.set(cond.col1);

      if (isHighlight) {
        // Extra vibrant for active branch
        SCRATCH_COLOR_1.multiplyScalar(1.25);
        SCRATCH_COLOR_2.multiplyScalar(1.25);
      }

      colArray[writePtr + 0] = SCRATCH_COLOR_1.r;
      colArray[writePtr + 1] = SCRATCH_COLOR_1.g;
      colArray[writePtr + 2] = SCRATCH_COLOR_1.b;
      colArray[writePtr + 3] = SCRATCH_COLOR_2.r;
      colArray[writePtr + 4] = SCRATCH_COLOR_2.g;
      colArray[writePtr + 5] = SCRATCH_COLOR_2.b;

      writePtr += 6;
    });

    // 2. Write Graph Links — skip if either endpoint is filtered out
    gLinks.forEach((link) => {
      const tgt = gNodes[link.targetIndex];
      if (!tgt) return;

      // Skip CORE_IDENTITY links only when a collection filter makes them irrelevant
      // (they connect personal nodes to the core — always draw unless search hides target)
      if (link.relation !== "CORE_IDENTITY") {
        // Check target node passes filters
        if (!isNodeLive(tgt)) return;

        // Check source node passes filters (if it's a real node, not the core or an anchor)
        if (link.sourceIndex >= 0 && link.sourceIndex < gNodes.length) {
          const srcNode = gNodes[link.sourceIndex];
          if (srcNode && !isNodeLive(srcNode)) return;
        }
      } else if (hasSearch) {
        // For CORE_IDENTITY links, still hide if target doesn't match search
        if (!isNodeLive(tgt)) return;
      }

      const tgtSessionId = tgt.sessionId ?? (tgt.factRecord?.session_id !== null ? String(tgt.factRecord.session_id) : null);
      const isSessionBranch = selSessionId !== null && (
        tgtSessionId === selSessionId ||
        link.fromId === `anchor_${selSessionId}` ||
        link.toId === `anchor_${selSessionId}`
      );

      // Invariant: When a session is selected, all links outside that session branch (except CORE_IDENTITY) VANISH completely!
      if (selSessionId !== null && !isSessionBranch && link.relation !== "CORE_IDENTITY") {
        return;
      }

      let srcX = 0, srcY = 0, srcZ = 0;
      if (link.sourceIndex === -1) {
        srcX = 0;
        srcY = 0;
        srcZ = 0;
      } else if (link.sourceIndex === -2) {
        const anchor = anchorMap.get(link.fromId);
        if (anchor) {
          srcX = anchor.x;
          srcY = anchor.y;
          srcZ = anchor.z;
        }
      } else if (link.sourceIndex >= 0 && link.sourceIndex < gNodes.length) {
        const srcNode = gNodes[link.sourceIndex];
        if (srcNode) {
          srcX = srcNode.x;
          srcY = srcNode.y;
          srcZ = srcNode.z;
        }
      }

      posArray[writePtr + 0] = srcX;
      posArray[writePtr + 1] = srcY;
      posArray[writePtr + 2] = srcZ;
      posArray[writePtr + 3] = tgt.x;
      posArray[writePtr + 4] = tgt.y;
      posArray[writePtr + 5] = tgt.z;

      const isFactConnected = selFactId !== null && (link.fromId === selFactId || link.toId === selFactId);

      // Sibling edge within the same session as the selected node
      const isSameSessionAsSelectedNode = selNodeSessionId !== null && tgtSessionId === selNodeSessionId;

      // Dimming rule: only applies when a fact is selected inside the session
      const isDimmed = selFactId !== null && isSameSessionAsSelectedNode && !isFactConnected && link.relation !== "CORE_IDENTITY";

      SCRATCH_COLOR_1.set(link.color);

      if (isDimmed) {
        const dimHex = isLight ? "#cbd5e1" : "#1e293b";
        SCRATCH_COLOR_1.set(dimHex);
        const dimFactor = isLight ? 0.35 : 0.15;
        colArray[writePtr + 0] = SCRATCH_COLOR_1.r * dimFactor;
        colArray[writePtr + 1] = SCRATCH_COLOR_1.g * dimFactor;
        colArray[writePtr + 2] = SCRATCH_COLOR_1.b * dimFactor;
        colArray[writePtr + 3] = SCRATCH_COLOR_1.r * dimFactor;
        colArray[writePtr + 4] = SCRATCH_COLOR_1.g * dimFactor;
        colArray[writePtr + 5] = SCRATCH_COLOR_1.b * dimFactor;
      } else if (isFactConnected) {
        // Boost selected fact link vibrancy
        SCRATCH_START_COLOR.copy(SCRATCH_COLOR_1).multiplyScalar(1.5);
        colArray[writePtr + 0] = SCRATCH_START_COLOR.r;
        colArray[writePtr + 1] = SCRATCH_START_COLOR.g;
        colArray[writePtr + 2] = SCRATCH_START_COLOR.b;
        colArray[writePtr + 3] = SCRATCH_COLOR_1.r;
        colArray[writePtr + 4] = SCRATCH_COLOR_1.g;
        colArray[writePtr + 5] = SCRATCH_COLOR_1.b;
      } else if (isSessionBranch) {
        // Boost selected branch link vibrancy
        SCRATCH_START_COLOR.copy(SCRATCH_COLOR_1).multiplyScalar(1.25);
        colArray[writePtr + 0] = SCRATCH_START_COLOR.r;
        colArray[writePtr + 1] = SCRATCH_START_COLOR.g;
        colArray[writePtr + 2] = SCRATCH_START_COLOR.b;
        colArray[writePtr + 3] = SCRATCH_COLOR_1.r;
        colArray[writePtr + 4] = SCRATCH_COLOR_1.g;
        colArray[writePtr + 5] = SCRATCH_COLOR_1.b;
      } else {
        // Keep true vibrant chromatic colors with gentle soft gradient.
        SCRATCH_START_COLOR.copy(SCRATCH_COLOR_1);
        if (isLight) {
          SCRATCH_START_COLOR.lerp(LIGHT_BG_COLOR, 0.12);
        } else {
          SCRATCH_START_COLOR.multiplyScalar(0.88);
        }

        colArray[writePtr + 0] = SCRATCH_START_COLOR.r;
        colArray[writePtr + 1] = SCRATCH_START_COLOR.g;
        colArray[writePtr + 2] = SCRATCH_START_COLOR.b;
        colArray[writePtr + 3] = SCRATCH_COLOR_1.r;
        colArray[writePtr + 4] = SCRATCH_COLOR_1.g;
        colArray[writePtr + 5] = SCRATCH_COLOR_1.b;
      }

      writePtr += 6;
    });

    posAttr.needsUpdate = true;
    colAttr.needsUpdate = true;
    geo.setDrawRange(0, writePtr / 3);
  }, []);

  // ── Dynamic Chronological Time-Tree Algorithm ──────────────────────────────
  const workerRef = useRef<Worker | null>(null);

  useEffect(() => {
    let worker: Worker | null = null;
    try {
      worker = new Worker(new URL("./memoryGraphWorker.ts", import.meta.url), {
        type: "module",
      });
      workerRef.current = worker;

      worker.onmessage = (e: MessageEvent<GraphWorkerOutput>) => {
        const { gNodes, gLinks, sessionAnchors, conduits } = e.data;
        gNodesRef.current = gNodes;
        gLinksRef.current = gLinks;
        sessionAnchorsRef.current = sessionAnchors;
        conduitsRef.current = conduits.map((c) => ({
          ...c,
          p0: new THREE.Vector3(c.p0.x, c.p0.y, c.p0.z),
          p1: new THREE.Vector3(c.p1.x, c.p1.y, c.p1.z),
        }));

        updateWebGLBuffers();
        updateLineHighlighting();
        wakeLoopRef.current();
      };

      if (factsRef.current && factsRef.current.length > 0) {
        const pal = getActiveDynamicPalette(isLightModeRef.current);
        worker.postMessage({
          facts: factsRef.current,
          isLight: isLightModeRef.current,
          dynamicPalette: pal,
          coreAccentHex: pal.personal.main,
        });
      }
    } catch (e) {
      console.error("[MemoryGraph] Failed to initialize worker:", e);
    }

    return () => {
      if (worker) {
        worker.terminate();
        workerRef.current = null;
      }
    };
  }, [updateLineHighlighting, updateWebGLBuffers]);

  const rebuildTopology = useCallback(
    (inputFacts: ObservationRecord[], isLight: boolean) => {
      const instancedMesh = instancedMeshRef.current;
      const instancedRing = instancedRingRef.current;
      const lineSegments = lineSegmentsRef.current;

      if (!inputFacts || inputFacts.length === 0) {
        gNodesRef.current = [];
        gLinksRef.current = [];
        sessionAnchorsRef.current = [];
        conduitsRef.current = [];
        if (instancedMesh) instancedMesh.count = 0;
        if (instancedRing) instancedRing.count = 0;
        if (lineSegments) lineSegments.geometry.setDrawRange(0, 0);
        return;
      }

      const dynamicPalette = getActiveDynamicPalette(isLight);
      const coreAccentHex = dynamicPalette.personal.main;

      if (workerRef.current) {
        workerRef.current.postMessage({
          facts: inputFacts,
          isLight,
          dynamicPalette,
          coreAccentHex,
        });
      }
    },
    []
  );

  // Re-run buffer updates when search/filter/selection changes
  useEffect(() => {
    updateWebGLBuffers();
    updateLineHighlighting();
    wakeLoopRef.current();
  }, [searchQuery, selectedCollection, selectedFactId, selectedSessionId, updateLineHighlighting, updateWebGLBuffers]);

  // Topology Update Effect: triggers whenever facts array or light mode changes
  useEffect(() => {
    rebuildTopologyRef.current = rebuildTopology;
    rebuildTopology(facts, isLightMode);
  }, [facts, isLightMode, rebuildTopology]);

  // Navigation helpers
  const recenter = useCallback(() => {
    flyToTargetRef.current = {
      cam: { x: 0, y: 0, z: 3100 },
      target: { x: 0, y: 0, z: 0 },
    };
    wakeLoopRef.current();
  }, []);

  const focusCore = useCallback(() => {
    flyToTargetRef.current = {
      cam: { x: 0, y: 0, z: 750 },
      target: { x: 0, y: 0, z: 0 },
    };
    wakeLoopRef.current();
  }, []);

  const flyToSession = useCallback((sessionId: string) => {
    const anchors = sessionAnchorsRef.current;
    const anchor = anchors.find((a) => a.sId === sessionId);
    if (!anchor) return;

    const anchorVec = new THREE.Vector3(anchor.x, anchor.y, anchor.z);
    const dist = anchorVec.length();
    const u = dist > 1 ? anchorVec.clone().normalize() : new THREE.Vector3(0, 0, 1);
    const camTarget = anchorVec.clone().addScaledVector(u, 440);

    flyToTargetRef.current = {
      cam: { x: camTarget.x, y: camTarget.y, z: camTarget.z },
      target: { x: 0, y: 0, z: 0 },
    };
    wakeLoopRef.current();
  }, []);

  const flyToNode = useCallback((factId: string) => {
    const gNodes = gNodesRef.current;
    const node = gNodes.find((n) => n.id === factId);
    if (!node) return;

    const nodeVec = new THREE.Vector3(node.x, node.y, node.z);
    const dist = nodeVec.length();
    const u = dist > 1 ? nodeVec.clone().normalize() : new THREE.Vector3(0, 0, 1);
    const camTarget = nodeVec.clone().addScaledVector(u, 380);

    flyToTargetRef.current = {
      cam: { x: camTarget.x, y: camTarget.y, z: camTarget.z },
      target: { x: 0, y: 0, z: 0 },
    };
    wakeLoopRef.current();
  }, []);

  const zoomIn = useCallback(() => {
    if (cameraRef.current && controlsRef.current) {
      const cam = cameraRef.current;
      const target = controlsRef.current.target;
      const dir = new THREE.Vector3().subVectors(target, cam.position).normalize();
      const dist = cam.position.distanceTo(target);
      if (dist > 300) {
        cam.position.addScaledVector(dir, Math.min(450, dist - 250));
        controlsRef.current.update();
        wakeLoopRef.current();
      }
    }
  }, []);

  const zoomOut = useCallback(() => {
    if (cameraRef.current && controlsRef.current) {
      const cam = cameraRef.current;
      const target = controlsRef.current.target;
      const dir = new THREE.Vector3().subVectors(cam.position, target).normalize();
      const dist = cam.position.distanceTo(target);
      if (dist < 16000) {
        cam.position.addScaledVector(dir, Math.min(450, 16500 - dist));
        controlsRef.current.update();
        wakeLoopRef.current();
      }
    }
  }, []);

  // Three.js Scene Setup (Mounts strictly ONCE)
  useEffect(() => {
    const container = canvasContainerRef.current;
    if (!container) return;

    const initialWidth = container.clientWidth || widthRef.current || 800;
    const initialHeight = container.clientHeight || heightRef.current || 600;

    // 1. Scene
    const scene = new THREE.Scene();
    sceneRef.current = scene;

    // 2. Camera — centered with calc(50% - 36px) offset to match Home Orb & History stage
    const camera = new THREE.PerspectiveCamera(55, initialWidth / initialHeight, 5, 30000);
    camera.setViewOffset(initialWidth, initialHeight, 0, 36, initialWidth, initialHeight);
    camera.position.set(0, 0, 3100);
    cameraRef.current = camera;

    // 3. Renderer
    const isGraphSoftwareRaster = isSoftwareRasterizer();
    const renderer = new THREE.WebGLRenderer({
      antialias: !isGraphSoftwareRaster,
      alpha: true,
      powerPreference: isGraphSoftwareRaster ? "low-power" : "high-performance",
    });
    renderer.setSize(initialWidth, initialHeight, false);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, isGraphSoftwareRaster ? 1 : 1.5));
    renderer.setClearColor(0x000000, 0);
    renderer.domElement.style.display = "block";
    renderer.domElement.style.width = "100%";
    renderer.domElement.style.height = "100%";
    container.appendChild(renderer.domElement);
    rendererRef.current = renderer;

    // 4. OrbitControls
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableRotate = true;
    controls.rotateSpeed = 0.65;
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.minDistance = 150;
    controls.maxDistance = 18000;
    controls.zoomSpeed = 0.85;
    controls.panSpeed = 0.8;
    controlsRef.current = controls;

    // 5. Lighting: Soft Ambient + Point Light at Center Core
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.6);
    scene.add(ambientLight);

    const initialPalette = getActiveDynamicPalette(isLightModeRef.current);
    const corePointLight = new THREE.PointLight(initialPalette.personal.threeColor, 2.2, 2800);
    corePointLight.position.set(0, 0, 0);
    scene.add(corePointLight);

    // 6. Sentient Personal Memory Core: Hollow Gradient Crystal Sphere + Geodesic Wireframe + Inner Nucleus
    const coreGeo = new THREE.SphereGeometry(74, 48, 48);
    const coreVertexShader = `
      varying vec3 vNormal;
      varying vec3 vPosition;
      void main() {
        vNormal = normalize(normalMatrix * normal);
        vPosition = (modelViewMatrix * vec4(position, 1.0)).xyz;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `;
    const coreFragmentShader = `
      uniform vec3 color1;
      uniform vec3 color2;
      uniform float opacity;
      varying vec3 vNormal;
      varying vec3 vPosition;
      void main() {
        vec3 viewDir = normalize(-vPosition);
        float fresnel = pow(1.0 - max(dot(viewDir, vNormal), 0.0), 2.2);
        float gradient = clamp(vNormal.y * 0.5 + 0.5, 0.0, 1.0);
        vec3 col = mix(color1, color2, gradient);
        float alpha = clamp(0.12 + fresnel * 0.78, 0.0, 1.0) * opacity;
        gl_FragColor = vec4(col * (1.0 + fresnel * 0.4), alpha);
      }
    `;
    const coreMat = new THREE.ShaderMaterial({
      vertexShader: coreVertexShader,
      fragmentShader: coreFragmentShader,
      uniforms: {
        color1: { value: new THREE.Color(isLightModeRef.current ? "#0284c7" : "#00f0ff") },
        color2: { value: new THREE.Color(isLightModeRef.current ? "#9333ea" : "#c084fc") },
        opacity: { value: isLightModeRef.current ? 0.85 : 0.75 },
      },
      transparent: true,
      side: THREE.FrontSide,
      blending: THREE.NormalBlending,
      depthWrite: false,
    });
    const coreMesh = new THREE.Mesh(coreGeo, coreMat);
    scene.add(coreMesh);
    coreMeshRef.current = coreMesh;

    // Outer Geodesic Wireframe
    const coreWireGeo = new THREE.IcosahedronGeometry(76, 3);
    const coreWireMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      wireframe: true,
      transparent: true,
      opacity: isLightModeRef.current ? 0.22 : 0.35,
      blending: isLightModeRef.current ? THREE.NormalBlending : THREE.AdditiveBlending,
    });
    const coreWireMesh = new THREE.Mesh(coreWireGeo, coreWireMat);
    scene.add(coreWireMesh);
    coreWireMeshRef.current = coreWireMesh;

    // Inner Luminous Nucleus
    const coreNucleusGeo = new THREE.SphereGeometry(24, 32, 32);
    const coreNucleusMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      transparent: true,
      opacity: isLightModeRef.current ? 0.90 : 0.80,
    });
    const coreNucleusMesh = new THREE.Mesh(coreNucleusGeo, coreNucleusMat);
    scene.add(coreNucleusMesh);
    coreNucleusMeshRef.current = coreNucleusMesh;

    // Concentric Orbital Celestial Rings
    const innerRingGeo = new THREE.RingGeometry(88, 92, 64);
    const innerRingMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      transparent: true,
      opacity: isLightModeRef.current ? 0.55 : 0.45,
      side: THREE.DoubleSide,
    });
    const innerRingMesh = new THREE.Mesh(innerRingGeo, innerRingMat);
    innerRingMesh.rotation.x = Math.PI * 0.35;
    scene.add(innerRingMesh);
    coreInnerRingRef.current = innerRingMesh;

    const outerRingGeo = new THREE.RingGeometry(105, 108, 64);
    const outerRingMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      transparent: true,
      opacity: isLightModeRef.current ? 0.40 : 0.30,
      side: THREE.DoubleSide,
    });
    const outerRingMesh = new THREE.Mesh(outerRingGeo, outerRingMat);
    outerRingMesh.rotation.z = -Math.PI * 0.25;
    scene.add(outerRingMesh);
    coreOuterRingRef.current = outerRingMesh;

    // 7. InstancedMesh for Fact Nodes (Jewel Spheres, Capacity 12,000)
    const maxNodes = 12000;
    const sphereGeo = new THREE.SphereGeometry(1, 16, 16);
    const nodeMat = new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: isLightModeRef.current ? 0.98 : 0.92,
    });
    const instancedMesh = new THREE.InstancedMesh(sphereGeo, nodeMat, maxNodes);
    instancedMesh.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(maxNodes * 3), 3);
    instancedMesh.count = 0;
    instancedMesh.frustumCulled = false;
    scene.add(instancedMesh);
    instancedMeshRef.current = instancedMesh;

    // 8. InstancedMesh for Glow Rings (Capacity 12,000)
    const ringGeo = new THREE.RingGeometry(1.18, 1.36, 32);
    const ringMat = new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: isLightModeRef.current ? 0.85 : 0.75,
      side: THREE.DoubleSide,
    });
    const instancedRing = new THREE.InstancedMesh(ringGeo, ringMat, maxNodes);
    instancedRing.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(maxNodes * 3), 3);
    instancedRing.count = 0;
    instancedRing.frustumCulled = false;
    scene.add(instancedRing);
    instancedRingRef.current = instancedRing;

    // 9. LineSegments for Neural Conduits (Capacity 40,000)
    const maxLines = 40000;
    const lineGeo = new THREE.BufferGeometry();
    lineGeo.setAttribute("position", new THREE.BufferAttribute(new Float32Array(maxLines * 6), 3));
    lineGeo.setAttribute("color", new THREE.BufferAttribute(new Float32Array(maxLines * 6), 3));

    const lineMat = new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: isLightModeRef.current ? 0.55 : 0.28,
      blending: THREE.NormalBlending,
      depthWrite: false,
    });
    const lineSegments = new THREE.LineSegments(lineGeo, lineMat);
    lineSegments.frustumCulled = false;
    scene.add(lineSegments);
    lineSegmentsRef.current = lineSegments;

    // If facts are already present, build topology immediately!
    if (factsRef.current && factsRef.current.length > 0) {
      rebuildTopologyRef.current?.(factsRef.current, isLightModeRef.current);
    }

    // 10. Dynamic FPS Animation Loop (60 FPS during camera motion / flyTo, 30 FPS when idle)
    const tempTargetVec = new THREE.Vector3();
    const tempCamVec = new THREE.Vector3();
    let lastRenderTimestamp = 0;
    let lastActivityTimestamp = performance.now();

    // ── PERF: idle settle (B4 — user-signed-off, revertible) ──────────
    // The graph now stops breathing AND stops rendering after
    // IDLE_SUSPEND_MS without interaction. Pointermove alone no longer keeps
    // it awake — only real intent does (drag with buttons held, wheel, touch,
    // camera change, fly-to, or data change via wakeLoopRef).
    // Previously the core pulse, wireframe rotation, nucleus pulse and both
    // rings ran unconditionally for 300ms→4000ms after every interaction, and
    // any cursor resting on the canvas held the loop at 30 FPS forever.
    const MOVE_WINDOW_MS = 300;

    const renderOnce = () => {
      if (rendererRef.current && sceneRef.current && cameraRef.current) {
        controls.update();
        rendererRef.current.render(sceneRef.current, cameraRef.current);
      }
    };
    renderOnceRef.current = renderOnce;

    const wakeLoop = () => {
      if (pausedRef.current) {
        renderOnce();
        return;
      }
      lastActivityTimestamp = performance.now();
      if (animFrameRef.current === null) {
        animFrameRef.current = requestAnimationFrame(render);
      }
    };
    wakeLoopRef.current = wakeLoop;

    const onControlsChange = () => {
      lastActivityTimestamp = performance.now();
      wakeLoop();
    };
    controls.addEventListener("change", onControlsChange);

    const domEl = renderer.domElement;
    const onVisibilityChange = () => {
      if (!document.hidden) wakeLoop();
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    // Drag with buttons held, wheel, touch, pointerdown, or camera change triggers active pacing
    const onPointerMoveWake = (e: PointerEvent) => {
      if (e.buttons & 1) wakeLoop();
    };
    domEl.addEventListener("pointermove", onPointerMoveWake, { passive: true });
    domEl.addEventListener("pointerdown", wakeLoop, { passive: true });
    domEl.addEventListener("wheel", wakeLoop, { passive: true });
    domEl.addEventListener("touchstart", wakeLoop, { passive: true });

    const render = (timestamp: number) => {
      // Skip render when tab/window is hidden
      if (document.hidden) {
        animFrameRef.current = null;
        return;
      }

      // Check if actively moving via flyTo lerp or recent control change
      const isMoving = Boolean(flyToTargetRef.current) || (timestamp - lastActivityTimestamp < MOVE_WINDOW_MS);

      // Idle suspend: park the loop entirely when nothing is moving. Pointer
      // intent (drag/wheel/touch/fly-to/data change) or a theme change wakes it.
      if (!isMoving) {
        animFrameRef.current = null;
        return;
      }

      animFrameRef.current = requestAnimationFrame(render);

      // 60 FPS while moving / flyTo; nothing to pace when idle (we are parked)
      const minInterval = 16;
      if (timestamp - lastRenderTimestamp < minInterval) return;
      lastRenderTimestamp = timestamp;

      const time = timestamp * 0.001;

      // Sentient breathing core — continuous organic breathing motion
      if (coreMeshRef.current) {
        const pulse = 1 + 0.035 * Math.sin(time * 2.2);
        coreMeshRef.current.scale.set(pulse, pulse, pulse);
      }
      if (coreWireMeshRef.current) {
        coreWireMeshRef.current.rotation.y = time * 0.12;
        coreWireMeshRef.current.rotation.x = time * 0.08;
      }
      if (coreNucleusMeshRef.current) {
        const nucPulse = 1 + 0.06 * Math.sin(time * 3.0);
        coreNucleusMeshRef.current.scale.set(nucPulse, nucPulse, nucPulse);
      }

      // Smooth harmonic counter-rotating rings
      if (coreInnerRingRef.current) {
        coreInnerRingRef.current.rotation.y = time * 0.22;
      }
      if (coreOuterRingRef.current) {
        coreOuterRingRef.current.rotation.y = -time * 0.16;
        coreOuterRingRef.current.rotation.x = Math.sin(time * 0.1) * 0.2;
      }

      // Smooth Camera Fly-To Lerp
      if (flyToTargetRef.current && cameraRef.current && controlsRef.current) {
        const flyTarget = flyToTargetRef.current;
        const cam = cameraRef.current;
        const ctrl = controlsRef.current;

        tempTargetVec.set(flyTarget.target.x, flyTarget.target.y, flyTarget.target.z);
        tempCamVec.set(flyTarget.cam.x, flyTarget.cam.y, flyTarget.cam.z);

        ctrl.target.lerp(tempTargetVec, 0.08);
        cam.position.lerp(tempCamVec, 0.08);

        if (cam.position.distanceTo(tempCamVec) < 2) {
          flyToTargetRef.current = null;
        }
      }

      controls.update();
      renderer.render(scene, camera);
    };

    animFrameRef.current = requestAnimationFrame(render);

    // Teardown
    return () => {
      wakeLoopRef.current = () => {};
      renderOnceRef.current = () => {};
      controls.removeEventListener("change", onControlsChange);
      domEl.removeEventListener("pointermove", onPointerMoveWake);
      domEl.removeEventListener("pointerdown", wakeLoop);
      domEl.removeEventListener("wheel", wakeLoop);
      domEl.removeEventListener("touchstart", wakeLoop);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      if (animFrameRef.current !== null) {
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = null;
      }
      controls.dispose();
      if (renderer.domElement) {
        renderer.domElement.width = 1;
        renderer.domElement.height = 1;
      }
      try {
        const gl = renderer.getContext();
        if (gl && !gl.isContextLost()) {
          renderer.forceContextLoss();
        }
      } catch {
        // Context may already be dropped by browser
      }
      renderer.dispose();
      if (renderer.domElement.parentNode) {
        renderer.domElement.parentNode.removeChild(renderer.domElement);
      }
      gNodesRef.current = [];
      gLinksRef.current = [];
      conduitsRef.current = [];
      sessionAnchorsRef.current = [];
      instancedMesh.dispose();
      instancedRing.dispose();
      sphereGeo.dispose();
      nodeMat.dispose();
      ringGeo.dispose();
      ringMat.dispose();
      lineGeo.dispose();
      lineMat.dispose();
      coreGeo.dispose();
      coreMat.dispose();
      coreWireGeo.dispose();
      coreWireMat.dispose();
      coreNucleusGeo.dispose();
      coreNucleusMat.dispose();
      innerRingGeo.dispose();
      innerRingMat.dispose();
      outerRingGeo.dispose();
      outerRingMat.dispose();
    };
  }, [canvasContainerRef]);

  // Resize Effect: guarantees distortion-free aspect ratio during maximize, restore, or window resize
  useEffect(() => {
    const updateSize = () => {
      const renderer = rendererRef.current;
      const camera = cameraRef.current;
      const scene = sceneRef.current;
      const container = canvasContainerRef.current;
      if (!renderer || !camera || !scene) return;

      const w = container?.clientWidth || width;
      const h = container?.clientHeight || height;
      if (w <= 0 || h <= 0) return;

      camera.aspect = w / h;
      camera.setViewOffset(w, h, 0, 36, w, h);
      camera.updateProjectionMatrix();
      renderer.setSize(w, h, false);
      const isGraphSoftwareRaster = isSoftwareRasterizer();
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, isGraphSoftwareRaster ? 1 : 1.5));

      // Immediate render of the new frame prevents stretched bitmap artifacts
      renderer.render(scene, camera);
      wakeLoopRef.current();
    };

    updateSize();

    const container = canvasContainerRef.current;
    if (!container) return;

    let rAfId: number | null = null;
    const ro = new ResizeObserver(() => {
      if (rAfId !== null) cancelAnimationFrame(rAfId);
      rAfId = requestAnimationFrame(updateSize);
    });
    ro.observe(container);

    return () => {
      ro.disconnect();
      if (rAfId !== null) cancelAnimationFrame(rAfId);
    };
  }, [canvasContainerRef, width, height]);

  return {
    isLightMode,
    gNodesRef,
    cameraRef,
    rendererRef,
    instancedMeshRef,
    coreMeshRef,
    recenter,
    focusCore,
    flyToSession,
    flyToNode,
    zoomIn,
    zoomOut,
  };
}
