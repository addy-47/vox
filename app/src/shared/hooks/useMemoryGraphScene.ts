import { useCallback, useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { FactRecord } from "@/services/memoryService";
import {
  GNode,
  GLink,
  MemoryCategory,
  getCollectionColor,
  getActiveDynamicPalette,
} from "@/shared/components/memory/memoryGraphTypes";

interface UseMemoryGraphSceneOptions {
  canvasContainerRef: React.RefObject<HTMLDivElement | null>;
  facts: FactRecord[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  selectedFactId: string | null;
  selectedSessionId?: string | null;
  onCoreClick?: () => void;
  clearCacheOnUnmount?: boolean;
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

export function useMemoryGraphScene({
  canvasContainerRef,
  facts,
  width,
  height,
  searchQuery,
  selectedCollection,
  selectedFactId,
  selectedSessionId = null,
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
  const coreInnerRingRef = useRef<THREE.Mesh | null>(null);
  const coreOuterRingRef = useRef<THREE.Mesh | null>(null);

  // Graph Data Refs
  const gNodesRef = useRef<GNode[]>([]);
  const gLinksRef = useRef<GLink[]>([]);
  const sessionAnchorsRef = useRef<SessionAnchor[]>([]);
  const conduitsRef = useRef<ConduitSegment[]>([]);
  const animFrameRef = useRef<number | null>(null);
  const flyToTargetRef = useRef<{ x: number; y: number; z: number } | null>(null);

  // Scratch Math Objects
  const dummyObjRef = useRef(new THREE.Object3D());
  const colorObjRef = useRef(new THREE.Color());

  // Prop Mirror Refs
  const selectedFactIdRef = useRef(selectedFactId);
  selectedFactIdRef.current = selectedFactId;

  const selectedSessionIdRef = useRef(selectedSessionId);
  selectedSessionIdRef.current = selectedSessionId;

  const searchQueryRef = useRef(searchQuery);
  searchQueryRef.current = searchQuery;

  const selectedCollectionRef = useRef(selectedCollection);
  selectedCollectionRef.current = selectedCollection;

  const factsRef = useRef(facts);
  factsRef.current = facts;

  const isLightModeRef = useRef(isLightMode);
  isLightModeRef.current = isLightMode;

  // Detect dark / light mode and dynamic accent changes
  useEffect(() => {
    const updateTheme = () => {
      const isLight = document.documentElement.getAttribute("data-theme") === "light";
      setIsLightMode(isLight);
      isLightModeRef.current = isLight;

      const palette = getActiveDynamicPalette(isLight);

      if (coreMeshRef.current) {
        const mat = coreMeshRef.current.material as THREE.MeshBasicMaterial;
        mat.color.copy(palette.personal.threeColor);
        mat.opacity = isLight ? 0.95 : 0.90;
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
        lineMat.blending = isLight ? THREE.NormalBlending : THREE.AdditiveBlending;
        lineMat.opacity = isLight ? 0.65 : 0.42;
        lineMat.needsUpdate = true;
      }
      if (instancedMeshRef.current) {
        const nodeMat = instancedMeshRef.current.material as THREE.MeshBasicMaterial;
        nodeMat.opacity = isLight ? 0.98 : 0.92;
        nodeMat.needsUpdate = true;
      }
    };

    updateTheme();
    const observer = new MutationObserver(updateTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "style", "class"],
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

    gNodes.forEach((node, i) => {
      const visible = isNodeVisible(node);
      const matchesSearch = !hasSearch || isNodeMatchingSearch(node);
      const isSelected = selFactId === node.id;
      const nodeSessionId = node.factRecord.session_id !== null ? String(node.factRecord.session_id) : null;
      const isSessionSelected = selSessionId !== null && nodeSessionId === selSessionId;
      const isOtherSession = selSessionId !== null && nodeSessionId !== selSessionId && node.collection !== "personal";

      const palette = getCollectionColor(node.collection, false, isLight);

      let radius: number;
      let colHex: string;

      if (!visible) {
        radius = 0.001;
        colHex = palette.main;
      } else if (hasSearch && !matchesSearch) {
        radius = 1.2;
        colHex = isLight ? "#94a3b8" : "#283344";
      } else if (isOtherSession) {
        // Gracefully dimmed non-selected session facts
        radius = 2.4;
        colHex = isLight ? "#cbd5e1" : "#1e293b";
      } else if (isSessionSelected) {
        // Highlighted session nodes
        radius = isSelected ? 12 : node.collection === "personal" ? 6.0 : 5.0;
        colHex = palette.main;
      } else {
        // Normal state
        radius = isSelected ? 12 : node.collection === "personal" ? 5.5 : 3.8;
        colHex = palette.main;
      }

      // Position Node
      dummy.position.set(node.x, node.y, node.z);
      dummy.scale.set(radius, radius, radius);
      dummy.updateMatrix();
      instancedMesh.setMatrixAt(i, dummy.matrix);

      color.set(colHex);
      instancedMesh.setColorAt(i, color);

      // Ring Halo Matrix (only active for selected facts or highlighted nodes)
      if (isSelected || (isSessionSelected && node.collection === "personal")) {
        const ringScale = radius * 2.8;
        dummy.scale.set(ringScale, ringScale, ringScale);
        dummy.lookAt(cameraRef.current ? cameraRef.current.position : new THREE.Vector3(0, 0, 3000));
        dummy.updateMatrix();
        instancedRing.setMatrixAt(i, dummy.matrix);
      } else {
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
    const isLight = isLightModeRef.current;

    const totalLines = conduits.length + gLinks.length;
    const posArray = new Float32Array(totalLines * 6);
    const colArray = new Float32Array(totalLines * 6);

    const colorHelper = new THREE.Color();
    const colorHelper2 = new THREE.Color();
    let writePtr = 0;

    // 1. Write Umbilical Conduits
    conduits.forEach((cond) => {
      posArray[writePtr + 0] = cond.p0.x;
      posArray[writePtr + 1] = cond.p0.y;
      posArray[writePtr + 2] = cond.p0.z;
      posArray[writePtr + 3] = cond.p1.x;
      posArray[writePtr + 4] = cond.p1.y;
      posArray[writePtr + 5] = cond.p1.z;

      const isHighlight = selSessionId !== null && cond.sessionId === selSessionId;
      const isDimmed = selSessionId !== null && cond.sessionId !== selSessionId;

      colorHelper.set(cond.col0);
      colorHelper2.set(cond.col1);

      if (isDimmed) {
        const dimHex = isLight ? "#cbd5e1" : "#1e293b";
        colorHelper.set(dimHex);
        colorHelper2.set(dimHex);
      } else if (isHighlight) {
        // Extra vibrant for active branch
        colorHelper.multiplyScalar(1.2);
        colorHelper2.multiplyScalar(1.2);
      }

      colArray[writePtr + 0] = colorHelper.r;
      colArray[writePtr + 1] = colorHelper.g;
      colArray[writePtr + 2] = colorHelper.b;
      colArray[writePtr + 3] = colorHelper2.r;
      colArray[writePtr + 4] = colorHelper2.g;
      colArray[writePtr + 5] = colorHelper2.b;

      writePtr += 6;
    });

    // 2. Write Graph Links
    gLinks.forEach((link) => {
      const tgt = gNodes[link.targetIndex];
      if (!tgt) return;

      let srcX = 0, srcY = 0, srcZ = 0;
      if (link.sourceIndex === -1) {
        srcX = 0;
        srcY = 0;
        srcZ = 0;
      } else if (link.sourceIndex === -2) {
        const sKey = link.fromId;
        const anchor = sessionAnchors.find((a) => a.sId === sKey);
        if (anchor) {
          srcX = anchor.x;
          srcY = anchor.y;
          srcZ = anchor.z;
        }
      }

      posArray[writePtr + 0] = srcX;
      posArray[writePtr + 1] = srcY;
      posArray[writePtr + 2] = srcZ;
      posArray[writePtr + 3] = tgt.x;
      posArray[writePtr + 4] = tgt.y;
      posArray[writePtr + 5] = tgt.z;

      const isHighlight = selSessionId !== null && link.fromId === selSessionId;
      const isDimmed = selSessionId !== null && link.fromId !== selSessionId && link.relation !== "CORE_IDENTITY";

      colorHelper.set(link.color);

      if (isDimmed) {
        const dimHex = isLight ? "#94a3b8" : "#1e293b";
        colorHelper.set(dimHex);
        colArray[writePtr + 0] = colorHelper.r * 0.3;
        colArray[writePtr + 1] = colorHelper.g * 0.3;
        colArray[writePtr + 2] = colorHelper.b * 0.3;
        colArray[writePtr + 3] = colorHelper.r * 0.3;
        colArray[writePtr + 4] = colorHelper.g * 0.3;
        colArray[writePtr + 5] = colorHelper.b * 0.3;
      } else {
        const fade = isHighlight ? 0.75 : 0.45;
        colArray[writePtr + 0] = colorHelper.r * fade;
        colArray[writePtr + 1] = colorHelper.g * fade;
        colArray[writePtr + 2] = colorHelper.b * fade;
        colArray[writePtr + 3] = colorHelper.r;
        colArray[writePtr + 4] = colorHelper.g;
        colArray[writePtr + 5] = colorHelper.b;
      }

      writePtr += 6;
    });

    const lineGeo = lineSegments.geometry;
    lineGeo.setAttribute("position", new THREE.BufferAttribute(posArray.subarray(0, writePtr), 3));
    lineGeo.setAttribute("color", new THREE.BufferAttribute(colArray.subarray(0, writePtr), 3));
    lineGeo.setDrawRange(0, writePtr / 3);
    lineGeo.attributes.position.needsUpdate = true;
    lineGeo.attributes.color.needsUpdate = true;
  }, []);

  // ── Dynamic Chronological Time-Tree Algorithm ──────────────────────────────
  const rebuildTopology = useCallback(
    (inputFacts: FactRecord[], isLight: boolean) => {
      const instancedMesh = instancedMeshRef.current;
      const instancedRing = instancedRingRef.current;
      const lineSegments = lineSegmentsRef.current;
      if (!instancedMesh || !instancedRing || !lineSegments) return;

      if (!inputFacts || inputFacts.length === 0) {
        gNodesRef.current = [];
        gLinksRef.current = [];
        sessionAnchorsRef.current = [];
        conduitsRef.current = [];
        instancedMesh.count = 0;
        instancedRing.count = 0;
        lineSegments.geometry.setDrawRange(0, 0);
        return;
      }

      const gNodes: GNode[] = [];
      const gLinks: GLink[] = [];
      const sessionAnchors: SessionAnchor[] = [];
      const conduits: ConduitSegment[] = [];

      // 1. Partition facts into Identity (Personal) and Session facts
      const identityFacts: FactRecord[] = [];
      const sessionMap = new Map<string, FactRecord[]>();

      inputFacts.forEach((fact) => {
        const cat = (fact.fact_type as MemoryCategory) || "objective";
        if (cat === "personal") {
          identityFacts.push(fact);
        } else {
          const sKey = fact.session_id !== null ? String(fact.session_id) : "unassigned";
          const list = sessionMap.get(sKey) ?? [];
          list.push(fact);
          sessionMap.set(sKey, list);
        }
      });

      // 2. Ring 1: Celestial Inner Halo of Identity Facts (R = 340)
      const rIdentity = 340;
      const nIdent = identityFacts.length;
      const phiWeight = (1 + Math.sqrt(5)) / 2;

      identityFacts.forEach((fact, i) => {
        const theta = (2 * Math.PI * i) / phiWeight;
        const phi = Math.acos(1 - (2 * (i + 0.5)) / Math.max(nIdent, 1));

        const x = rIdentity * Math.sin(phi) * Math.cos(theta);
        const y = (rIdentity * 0.72) * Math.sin(phi) * Math.sin(theta);
        const z = (rIdentity * 0.88) * Math.cos(phi);

        const colPalette = getCollectionColor("personal", false, isLight);

        gNodes.push({
          id: fact.id,
          label: fact.text,
          compactId: fact.id.slice(0, 6),
          collection: "personal",
          status: "active",
          factRecord: fact,
          color: colPalette.main,
          degree: 1,
          x,
          y,
          z,
          vx: 0,
          vy: 0,
          vz: 0,
        });

        gLinks.push({
          id: `link_ident_${fact.id}`,
          sourceIndex: -1,
          targetIndex: gNodes.length - 1,
          fromId: "core",
          toId: fact.id,
          relation: "CORE_IDENTITY",
          color: colPalette.main,
          isDashed: false,
        });
      });

      // 3. Hierarchical Chronological Time-Tree Branching
      // Sort sessions chronologically by earliest fact creation
      const sortedSessionKeys = Array.from(sessionMap.keys()).sort((a, b) => {
        const factsA = sessionMap.get(a)!;
        const factsB = sessionMap.get(b)!;
        const minA = Math.min(...factsA.map((f) => f.created_at || 0));
        const minB = Math.min(...factsB.map((f) => f.created_at || 0));
        return minA - minB;
      });

      const nSessions = sortedSessionKeys.length;
      const dynamicPalette = getActiveDynamicPalette(isLight);
      const coreAccentHex = dynamicPalette.personal.main;

      // Group sessions into Primary Temporal Limbs (e.g. Months/Quarters)
      const numTrunks = Math.min(Math.max(Math.ceil(nSessions / 8), 3), 7);
      const sessionsPerTrunk = Math.ceil(nSessions / numTrunks);

      sortedSessionKeys.forEach((sKey, sIdx) => {
        const clusterFacts = sessionMap.get(sKey)!;
        const nCluster = clusterFacts.length;
        const sessionTime = clusterFacts[0]?.created_at || sIdx;

        // Determine which primary temporal trunk this session belongs to
        const trunkIndex = Math.min(Math.floor(sIdx / sessionsPerTrunk), numTrunks - 1);
        const indexInTrunk = sIdx % sessionsPerTrunk;
        const trunkProgress = indexInTrunk / Math.max(sessionsPerTrunk - 1, 1);

        // Base azimuth angle for primary trunk (spread evenly around 360 degrees)
        const trunkBaseAngle = (trunkIndex / numTrunks) * Math.PI * 2;
        // Natural gentle organic elevation tilt
        const trunkPitch = (trunkIndex % 2 === 0 ? 1 : -1) * (0.18 + 0.12 * Math.sin(trunkIndex));

        // Trunk Fork waypoint (R = 580)
        const rFork = 580;
        const forkX = rFork * Math.cos(trunkBaseAngle) * Math.cos(trunkPitch);
        const forkY = rFork * Math.sin(trunkPitch) * 0.8;
        const forkZ = rFork * Math.sin(trunkBaseAngle) * Math.cos(trunkPitch);
        const forkPoint = new THREE.Vector3(forkX, forkY, forkZ);

        // Secondary Branch fanout angle (sessions within the same limb cluster together)
        const fanSpread = 0.55; // Radians
        const sessionAngle = trunkBaseAngle + (trunkProgress - 0.5) * fanSpread;
        const sessionPitch = trunkPitch + ((sIdx % 3) - 1) * 0.18;

        // Session Anchor nexus position (R = 1150 - 1350)
        const rSession = 1150 + (sIdx % 4) * 65;
        const trunkX = rSession * Math.cos(sessionAngle) * Math.cos(sessionPitch);
        const trunkY = rSession * Math.sin(sessionPitch) * 0.85;
        const trunkZ = rSession * Math.sin(sessionAngle) * Math.cos(sessionPitch);
        const sessionAnchor = new THREE.Vector3(trunkX, trunkY, trunkZ);

        const dominantCat = (clusterFacts[0]?.fact_type as MemoryCategory) || "objective";
        const clusterPalette = getCollectionColor(dominantCat, false, isLight);

        sessionAnchors.push({
          sId: sKey,
          count: nCluster,
          x: trunkX,
          y: trunkY,
          z: trunkZ,
          color: clusterPalette.main,
          timestamp: sessionTime,
        });

        // ── Smooth Bezier Neural Umbilical Conduits ──
        // Level 1: Core (0, 0, 0) -> Trunk Fork
        const numCoreSegs = 3;
        let prevP = new THREE.Vector3(0, 0, 0);
        for (let seg = 1; seg <= numCoreSegs; seg++) {
          const t = seg / numCoreSegs;
          const pt = new THREE.Vector3().lerpVectors(new THREE.Vector3(0, 0, 0), forkPoint, t);
          conduits.push({
            p0: prevP.clone(),
            p1: pt.clone(),
            col0: coreAccentHex,
            col1: coreAccentHex,
            sessionId: sKey,
          });
          prevP = pt;
        }

        // Level 2: Trunk Fork -> Session Anchor
        const numBranchSegs = 4;
        const branchMid = new THREE.Vector3().lerpVectors(forkPoint, sessionAnchor, 0.5);
        branchMid.y += (sIdx % 2 === 0 ? 35 : -35); // Organic curvature

        prevP = forkPoint.clone();
        for (let seg = 1; seg <= numBranchSegs; seg++) {
          const t = seg / numBranchSegs;
          // Quadratic bezier interpolation
          const pt = new THREE.Vector3()
            .copy(forkPoint).multiplyScalar((1 - t) * (1 - t))
            .addScaledVector(branchMid, 2 * (1 - t) * t)
            .addScaledVector(sessionAnchor, t * t);

          conduits.push({
            p0: prevP.clone(),
            p1: pt.clone(),
            col0: coreAccentHex,
            col1: clusterPalette.main,
            sessionId: sKey,
          });
          prevP = pt;
        }

        // ── Level 3: Dendritic Foliage (Facts branching from Session Anchor) ──
        const trunkVec = sessionAnchor.clone().normalize();
        const upVec = Math.abs(trunkVec.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0);
        const rightVec = new THREE.Vector3().crossVectors(trunkVec, upVec).normalize();
        const perpVec = new THREE.Vector3().crossVectors(trunkVec, rightVec).normalize();

        clusterFacts.forEach((fact, fIdx) => {
          const cat = (fact.fact_type as MemoryCategory) || "objective";
          const palette = getCollectionColor(cat, false, isLight);

          // Conical arbor spreading from session anchor
          const coneAngle = 0.16 + 0.50 * Math.sqrt((fIdx + 1) / nCluster);
          const coneRot = fIdx * 2.39996;
          const branchDist = 65 + 240 * Math.pow((fIdx + 1) / nCluster, 0.65);

          const radialOff = Math.sin(coneAngle) * branchDist;
          const axialOff = Math.cos(coneAngle) * branchDist;

          const posX = trunkX + axialOff * trunkVec.x + radialOff * (Math.cos(coneRot) * rightVec.x + Math.sin(coneRot) * perpVec.x);
          const posY = trunkY + axialOff * trunkVec.y + radialOff * (Math.cos(coneRot) * rightVec.y + Math.sin(coneRot) * perpVec.y);
          const posZ = trunkZ + axialOff * trunkVec.z + radialOff * (Math.cos(coneRot) * rightVec.z + Math.sin(coneRot) * perpVec.z);

          const nodeIdx = gNodes.length;
          gNodes.push({
            id: fact.id,
            label: fact.text,
            compactId: fact.id.slice(0, 6),
            collection: cat,
            status: "active",
            factRecord: fact,
            color: palette.main,
            degree: 1,
            x: posX,
            y: posY,
            z: posZ,
            vx: 0,
            vy: 0,
            vz: 0,
          });

          gLinks.push({
            id: `link_branch_${fact.id}`,
            sourceIndex: -2,
            targetIndex: nodeIdx,
            fromId: sKey,
            toId: fact.id,
            relation: cat.toUpperCase(),
            color: palette.main,
            isDashed: false,
          });
        });
      });

      gNodesRef.current = gNodes;
      gLinksRef.current = gLinks;
      sessionAnchorsRef.current = sessionAnchors;
      conduitsRef.current = conduits;

      updateWebGLBuffers();
      updateLineHighlighting();
    },
    [updateLineHighlighting, updateWebGLBuffers]
  );

  // Re-run buffer updates when search/filter/selection changes
  useEffect(() => {
    updateWebGLBuffers();
    updateLineHighlighting();
  }, [searchQuery, selectedCollection, selectedFactId, selectedSessionId, updateLineHighlighting, updateWebGLBuffers]);

  // Topology Update Effect: triggers whenever facts array or light mode changes
  useEffect(() => {
    rebuildTopology(facts, isLightMode);
  }, [facts, isLightMode, rebuildTopology]);

  // Smooth Camera Fly-To Lerp when selectedFactId changes
  useEffect(() => {
    if (!selectedFactId) return;
    const gNodes = gNodesRef.current;
    const node = gNodes.find((n) => n.id === selectedFactId);
    if (!node) return;

    flyToTargetRef.current = {
      x: node.x,
      y: node.y,
      z: node.z + 350,
    };
  }, [selectedFactId]);

  // Navigation helpers
  const recenter = useCallback(() => {
    flyToTargetRef.current = { x: 0, y: 0, z: 3100 };
  }, []);

  const focusCore = useCallback(() => {
    flyToTargetRef.current = { x: 0, y: 0, z: 750 };
  }, []);

  const flyToSession = useCallback((sessionId: string) => {
    const anchors = sessionAnchorsRef.current;
    const anchor = anchors.find((a) => a.sId === sessionId);
    if (!anchor) return;

    flyToTargetRef.current = {
      x: anchor.x,
      y: anchor.y,
      z: anchor.z + 450,
    };
  }, []);

  const flyToNode = useCallback((factId: string) => {
    const gNodes = gNodesRef.current;
    const node = gNodes.find((n) => n.id === factId);
    if (!node) return;

    flyToTargetRef.current = {
      x: node.x,
      y: node.y,
      z: node.z + 320,
    };
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
        cam.position.addScaledVector(dir, 450);
        controlsRef.current.update();
      }
    }
  }, []);

  // Three.js Scene Setup (Mounts strictly ONCE)
  useEffect(() => {
    const container = canvasContainerRef.current;
    if (!container) return;

    const initialWidth = container.clientWidth || width || 800;
    const initialHeight = container.clientHeight || height || 600;

    // 1. Scene
    const scene = new THREE.Scene();
    sceneRef.current = scene;

    // 2. Camera
    const camera = new THREE.PerspectiveCamera(55, initialWidth / initialHeight, 5, 30000);
    camera.position.set(0, 0, 3100);
    cameraRef.current = camera;

    // 3. Renderer
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setSize(initialWidth, initialHeight);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setClearColor(0x000000, 0);
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

    // 6. Sentient Personal Memory Core (Volumetric Sphere + Dual Orbital Rings)
    const coreGeo = new THREE.SphereGeometry(74, 48, 48);
    const coreMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      transparent: true,
      opacity: isLightModeRef.current ? 0.95 : 0.90,
    });
    const coreMesh = new THREE.Mesh(coreGeo, coreMat);
    scene.add(coreMesh);
    coreMeshRef.current = coreMesh;

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
    instancedMesh.count = 0;
    instancedMesh.frustumCulled = false;
    scene.add(instancedMesh);
    instancedMeshRef.current = instancedMesh;

    // 8. InstancedMesh for Glow Rings (Capacity 12,000)
    const ringGeo = new THREE.RingGeometry(1, 1.45, 24);
    const ringMat = new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: isLightModeRef.current ? 0.50 : 0.42,
      side: THREE.DoubleSide,
    });
    const instancedRing = new THREE.InstancedMesh(ringGeo, ringMat, maxNodes);
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
      opacity: isLightModeRef.current ? 0.65 : 0.42,
      blending: isLightModeRef.current ? THREE.NormalBlending : THREE.AdditiveBlending,
      depthWrite: false,
    });
    const lineSegments = new THREE.LineSegments(lineGeo, lineMat);
    lineSegments.frustumCulled = false;
    scene.add(lineSegments);
    lineSegmentsRef.current = lineSegments;

    // If facts are already present, build topology immediately!
    if (factsRef.current && factsRef.current.length > 0) {
      rebuildTopology(factsRef.current, isLightModeRef.current);
    }

    // 10. Continuous 60 FPS Animation Loop
    const tempTargetVec = new THREE.Vector3();
    const tempCamVec = new THREE.Vector3();

    const render = () => {
      animFrameRef.current = requestAnimationFrame(render);
      const time = performance.now() * 0.001;

      // Sentient breathing core
      if (coreMeshRef.current) {
        const pulse = 1 + 0.035 * Math.sin(time * 2.2);
        coreMeshRef.current.scale.set(pulse, pulse, pulse);
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
        const target = flyToTargetRef.current;
        const cam = cameraRef.current;
        const ctrl = controlsRef.current;

        tempTargetVec.set(target.x * 0.4, target.y * 0.4, target.z * 0.4);
        tempCamVec.set(target.x, target.y, target.z);

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
      if (animFrameRef.current !== null) {
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = null;
      }
      controls.dispose();
      renderer.forceContextLoss();
      renderer.dispose();
      if (renderer.domElement.parentNode) {
        renderer.domElement.parentNode.removeChild(renderer.domElement);
      }
      sphereGeo.dispose();
      nodeMat.dispose();
      ringGeo.dispose();
      ringMat.dispose();
      lineGeo.dispose();
      lineMat.dispose();
      coreGeo.dispose();
      coreMat.dispose();
      innerRingGeo.dispose();
      innerRingMat.dispose();
      outerRingGeo.dispose();
      outerRingMat.dispose();
    };
  }, []);

  // Resize Effect
  useEffect(() => {
    const renderer = rendererRef.current;
    const camera = cameraRef.current;
    if (!renderer || !camera || width === 0 || height === 0) return;

    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
  }, [width, height]);

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
