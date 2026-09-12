import { useCallback, useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { FactRecord } from "@/services/memoryService";
import {
  GNode,
  GLink,
  ClusterBadgeData,
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
}

export function useMemoryGraphScene({
  canvasContainerRef,
  facts,
  width,
  height,
  searchQuery,
  selectedCollection,
  selectedFactId,
}: UseMemoryGraphSceneOptions) {
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
  const coreMeshRef = useRef<THREE.Mesh | null>(null);
  const coreHaloRef = useRef<THREE.Mesh | null>(null);

  // Graph Data Refs
  const gNodesRef = useRef<GNode[]>([]);
  const gLinksRef = useRef<GLink[]>([]);
  const sessionAnchorsRef = useRef<SessionAnchor[]>([]);
  const animFrameRef = useRef<number | null>(null);
  const flyToTargetRef = useRef<{ x: number; y: number; z: number } | null>(null);
  const badgeTimerRef = useRef<NodeJS.Timeout | null>(null);

  // Scratch Math Objects
  const dummyObjRef = useRef(new THREE.Object3D());
  const colorObjRef = useRef(new THREE.Color());
  const tempVecRef = useRef(new THREE.Vector3());

  // Prop Mirror Refs
  const selectedFactIdRef = useRef(selectedFactId);
  selectedFactIdRef.current = selectedFactId;

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
        mat.opacity = isLight ? 0.95 : 0.88;
        mat.needsUpdate = true;
      }
      if (coreHaloRef.current) {
        const mat = coreHaloRef.current.material as THREE.MeshBasicMaterial;
        mat.color.copy(palette.personal.threeColor);
        mat.opacity = isLight ? 0.38 : 0.22;
        mat.needsUpdate = true;
      }
      if (lineSegmentsRef.current) {
        const lineMat = lineSegmentsRef.current.material as THREE.LineBasicMaterial;
        lineMat.blending = isLight ? THREE.NormalBlending : THREE.AdditiveBlending;
        lineMat.opacity = isLight ? 0.65 : 0.40;
        lineMat.needsUpdate = true;
      }
      if (instancedMeshRef.current) {
        const nodeMat = instancedMeshRef.current.material as THREE.MeshBasicMaterial;
        nodeMat.opacity = isLight ? 0.96 : 0.90;
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

  // Update WebGL InstancedMesh Buffers & Colors
  const updateWebGLBuffers = useCallback(() => {
    const gNodes = gNodesRef.current;
    const instancedMesh = instancedMeshRef.current;
    const instancedRing = instancedRingRef.current;
    if (!instancedMesh || !instancedRing || gNodes.length === 0) return;

    const dummy = dummyObjRef.current;
    const color = colorObjRef.current;
    const selFactId = selectedFactIdRef.current;
    const sq = searchQueryRef.current.trim().toLowerCase();
    const hasSearch = sq.length > 0;
    const isLight = isLightModeRef.current;

    gNodes.forEach((node, i) => {
      const visible = isNodeVisible(node);
      const matchesSearch = !hasSearch || isNodeMatchingSearch(node);
      const isSelected = selFactId === node.id;
      const palette = getCollectionColor(node.collection, false, isLight);

      let radius: number;
      let colHex: string;

      if (!visible) {
        radius = 0.001;
        colHex = palette.main;
      } else if (hasSearch && !matchesSearch) {
        radius = 1.5;
        colHex = isLight ? "#94a3b8" : "#283344";
      } else {
        radius = isSelected ? 11 : node.collection === "personal" ? 5.5 : 4.2;
        colHex = palette.main;
      }

      // Position Node
      dummy.position.set(node.x, node.y, node.z);
      dummy.scale.set(radius, radius, radius);
      dummy.updateMatrix();
      instancedMesh.setMatrixAt(i, dummy.matrix);

      color.set(colHex);
      instancedMesh.setColorAt(i, color);

      // Ring Halo Matrix
      const ringScale = isSelected ? radius * 2.6 : radius * 2.0;
      dummy.scale.set(ringScale, ringScale, ringScale);
      dummy.lookAt(cameraRef.current ? cameraRef.current.position : new THREE.Vector3(0, 0, 3000));
      dummy.updateMatrix();
      instancedRing.setMatrixAt(i, dummy.matrix);
    });

    instancedMesh.count = gNodes.length;
    instancedRing.count = gNodes.length;
    if (instancedMesh.instanceMatrix) instancedMesh.instanceMatrix.needsUpdate = true;
    if (instancedMesh.instanceColor) instancedMesh.instanceColor.needsUpdate = true;
    if (instancedRing.instanceMatrix) instancedRing.instanceMatrix.needsUpdate = true;
  }, [isNodeMatchingSearch, isNodeVisible]);

  // Project Centroid Badges to Screen Coordinates
  const projectCentroidBadges = useCallback(() => {
    const camera = cameraRef.current;
    const renderer = rendererRef.current;
    const anchors = sessionAnchorsRef.current;
    if (!camera || !renderer || anchors.length === 0) return;

    const badges: ClusterBadgeData[] = [];
    const tempVec = tempVecRef.current;
    const rect = renderer.domElement.getBoundingClientRect();

    for (const data of anchors) {
      if (data.count < 2 || data.sId === "unassigned") continue;

      tempVec.set(data.x, data.y, data.z);
      tempVec.project(camera);

      // Cull badges behind the camera or outside view frustum
      if (tempVec.z > 1.0 || tempVec.z < -1.0) continue;
      if (tempVec.x < -1.1 || tempVec.x > 1.1 || tempVec.y < -1.1 || tempVec.y > 1.1) continue;

      const screenX = (tempVec.x * 0.5 + 0.5) * rect.width;
      const screenY = (-(tempVec.y * 0.5) + 0.5) * rect.height;

      badges.push({
        collection: `Session #${data.sId}`,
        graphX: data.x,
        graphY: data.y,
        graphZ: data.z,
        screenX,
        screenY,
        factCount: data.count,
        color: data.color,
        desc: `${data.count} structured facts synthesized in session #${data.sId}.`,
      });

      if (badges.length >= 24) break;
    }

    setClusterBadges(badges);
  }, []);

  // Debounced Badge Update (avoids React re-renders during fast 60fps rotation)
  const scheduleBadgeUpdate = useCallback(
    (delayMs = 150) => {
      if (badgeTimerRef.current) clearTimeout(badgeTimerRef.current);
      badgeTimerRef.current = setTimeout(() => {
        projectCentroidBadges();
      }, delayMs);
    },
    [projectCentroidBadges]
  );

  // Core Topology Generator & Buffer Filler
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
        instancedMesh.count = 0;
        instancedRing.count = 0;
        lineSegments.geometry.setDrawRange(0, 0);
        setClusterBadges([]);
        return;
      }

      const gNodes: GNode[] = [];
      const gLinks: GLink[] = [];
      const sessionAnchors: SessionAnchor[] = [];

      // Partition facts into Ring 1 (personal identity) and Ring 2 (session clusters)
      const identityFacts: FactRecord[] = [];
      const sessionGroups = new Map<string, FactRecord[]>();

      inputFacts.forEach((fact) => {
        const cat = (fact.fact_type as MemoryCategory) || "objective";
        if (cat === "personal") {
          identityFacts.push(fact);
        } else {
          const sKey = fact.session_id !== null ? String(fact.session_id) : "unassigned";
          const group = sessionGroups.get(sKey);
          if (!group) {
            sessionGroups.set(sKey, [fact]);
          } else {
            group.push(fact);
          }
        }
      });

      // 1. Ring 1 (Celestial Inner Halo of Identity Facts): R = 340
      const rIdentity = 340;
      const nIdent = identityFacts.length;
      const phiWeight = (1 + Math.sqrt(5)) / 2; // Golden ratio

      identityFacts.forEach((fact, i) => {
        const theta = (2 * Math.PI * i) / phiWeight;
        const phi = Math.acos(1 - (2 * (i + 0.5)) / Math.max(nIdent, 1));

        const x = rIdentity * Math.sin(phi) * Math.cos(theta);
        const y = (rIdentity * 0.75) * Math.sin(phi) * Math.sin(theta);
        const z = (rIdentity * 0.85) * Math.cos(phi);

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

        // Link to Core (0, 0, 0)
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

      // 2. Ring 2 (Dendritic Session Constellations): R = 1200
      const rSessionBase = 1200;
      const sessionKeys = Array.from(sessionGroups.keys());
      const nSessions = sessionKeys.length;

      const trunkConduits: Array<{
        p0: THREE.Vector3;
        p1: THREE.Vector3;
        col0: string;
        col1: string;
      }> = [];

      const dynamicPalette = getActiveDynamicPalette(isLight);
      const coreAccentHex = dynamicPalette.personal.main;

      sessionKeys.forEach((sKey, sIdx) => {
        const sTheta = (2 * Math.PI * sIdx) / phiWeight;
        const sPhi = Math.acos(1 - (2 * (sIdx + 0.5)) / Math.max(nSessions, 1));

        const trunkDirX = Math.sin(sPhi) * Math.cos(sTheta);
        const trunkDirY = Math.sin(sPhi) * Math.sin(sTheta);
        const trunkDirZ = Math.cos(sPhi);

        const trunkX = rSessionBase * trunkDirX;
        const trunkY = (rSessionBase * 0.88) * trunkDirY;
        const trunkZ = (rSessionBase * 0.95) * trunkDirZ;

        const clusterFacts = sessionGroups.get(sKey) ?? [];
        const nCluster = clusterFacts.length;

        const dominantCat = (clusterFacts[0]?.fact_type as MemoryCategory) || "objective";
        const clusterPalette = getCollectionColor(dominantCat, false, isLight);

        sessionAnchors.push({
          sId: sKey,
          count: nCluster,
          x: trunkX,
          y: trunkY,
          z: trunkZ,
          color: clusterPalette.main,
        });

        // ── Physical Neural Umbilical Conduit: Core (0, 0, 0) -> Session Trunk ──
        const trunkEnd = new THREE.Vector3(trunkX, trunkY, trunkZ);
        const trunkVec = trunkEnd.clone().normalize();
        const upVec = Math.abs(trunkVec.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0);
        const rightVec = new THREE.Vector3().crossVectors(trunkVec, upVec).normalize();
        const perpVec = new THREE.Vector3().crossVectors(trunkVec, rightVec).normalize();

        const numTrunkSegs = 6;
        let prevPoint = new THREE.Vector3(0, 0, 0);
        let prevCompanion = new THREE.Vector3(0, 0, 0);

        for (let s = 1; s <= numTrunkSegs; s++) {
          const t = s / numTrunkSegs;
          const curDist = trunkEnd.length() * t;
          const arcAmount = Math.sin(t * Math.PI) * 85;
          const twistAmount = Math.sin(t * Math.PI * 2) * 35;

          const curPoint = trunkVec.clone().multiplyScalar(curDist)
            .addScaledVector(perpVec, arcAmount)
            .addScaledVector(rightVec, twistAmount * 0.4);

          const curCompanion = trunkVec.clone().multiplyScalar(curDist)
            .addScaledVector(perpVec, -arcAmount * 0.65)
            .addScaledVector(rightVec, -twistAmount * 0.5);

          // Primary trunk segment
          trunkConduits.push({
            p0: prevPoint.clone(),
            p1: s === numTrunkSegs ? trunkEnd.clone() : curPoint.clone(),
            col0: coreAccentHex,
            col1: clusterPalette.main,
          });

          // Braided companion filament
          trunkConduits.push({
            p0: prevCompanion.clone(),
            p1: s === numTrunkSegs ? trunkEnd.clone() : curCompanion.clone(),
            col0: coreAccentHex,
            col1: clusterPalette.main,
          });

          prevPoint = curPoint;
          prevCompanion = curCompanion;
        }

        // ── Dendritic Branching Arbors: Session Facts branching from Trunk ──
        clusterFacts.forEach((fact, fIdx) => {
          const cat = (fact.fact_type as MemoryCategory) || "objective";
          const palette = getCollectionColor(cat, false, isLight);

          const coneAngle = 0.18 + 0.52 * Math.sqrt((fIdx + 1) / nCluster);
          const coneRot = fIdx * 2.39996;
          const branchDist = 70 + 260 * Math.pow((fIdx + 1) / nCluster, 0.65);

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

          // Link to Session Trunk Anchor
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

      // Populate LineSegments geometry
      const totalLines = gLinks.length + trunkConduits.length;
      const posArray = new Float32Array(totalLines * 6);
      const colArray = new Float32Array(totalLines * 6);

      const colorHelper = new THREE.Color();
      const colorHelper2 = new THREE.Color();
      let writePtr = 0;

      // 1. Write Umbilical Conduits (Central Core to Session Trunks)
      trunkConduits.forEach((cond) => {
        posArray[writePtr + 0] = cond.p0.x;
        posArray[writePtr + 1] = cond.p0.y;
        posArray[writePtr + 2] = cond.p0.z;
        posArray[writePtr + 3] = cond.p1.x;
        posArray[writePtr + 4] = cond.p1.y;
        posArray[writePtr + 5] = cond.p1.z;

        colorHelper.set(cond.col0);
        colorHelper2.set(cond.col1);

        colArray[writePtr + 0] = colorHelper.r;
        colArray[writePtr + 1] = colorHelper.g;
        colArray[writePtr + 2] = colorHelper.b;
        colArray[writePtr + 3] = colorHelper2.r;
        colArray[writePtr + 4] = colorHelper2.g;
        colArray[writePtr + 5] = colorHelper2.b;

        writePtr += 6;
      });

      // 2. Write Graph Links (Identity to Core & Facts to Session Trunks)
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

        colorHelper.set(link.color);

        colArray[writePtr + 0] = colorHelper.r * 0.45;
        colArray[writePtr + 1] = colorHelper.g * 0.45;
        colArray[writePtr + 2] = colorHelper.b * 0.45;
        colArray[writePtr + 3] = colorHelper.r;
        colArray[writePtr + 4] = colorHelper.g;
        colArray[writePtr + 5] = colorHelper.b;

        writePtr += 6;
      });

      const lineGeo = lineSegments.geometry;
      lineGeo.setAttribute("position", new THREE.BufferAttribute(posArray.subarray(0, writePtr), 3));
      lineGeo.setAttribute("color", new THREE.BufferAttribute(colArray.subarray(0, writePtr), 3));
      lineGeo.setDrawRange(0, writePtr / 3);
      lineGeo.attributes.position.needsUpdate = true;
      lineGeo.attributes.color.needsUpdate = true;

      updateWebGLBuffers();
      scheduleBadgeUpdate(50);
    },
    [scheduleBadgeUpdate, updateWebGLBuffers]
  );

  // Re-run buffer updates when search/filter/selection changes
  useEffect(() => {
    updateWebGLBuffers();
  }, [searchQuery, selectedCollection, selectedFactId, updateWebGLBuffers]);

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
      z: node.z + 400,
    };
  }, [selectedFactId]);

  // Recenter helper: resets camera smoothly to front-view vantage
  const recenter = useCallback(() => {
    flyToTargetRef.current = { x: 0, y: 0, z: 3100 };
  }, []);

  // Focus Personal Core helper: brings camera up close to the central core
  const focusCore = useCallback(() => {
    flyToTargetRef.current = { x: 0, y: 0, z: 750 };
  }, []);

  // Step camera along view vector for accurate 3D zoom
  const zoomIn = useCallback(() => {
    if (cameraRef.current && controlsRef.current) {
      const cam = cameraRef.current;
      const target = controlsRef.current.target;
      const dir = new THREE.Vector3().subVectors(target, cam.position).normalize();
      const dist = cam.position.distanceTo(target);
      if (dist > 350) {
        cam.position.addScaledVector(dir, Math.min(450, dist - 300));
        controlsRef.current.update();
        scheduleBadgeUpdate(100);
      }
    }
  }, [scheduleBadgeUpdate]);

  const zoomOut = useCallback(() => {
    if (cameraRef.current && controlsRef.current) {
      const cam = cameraRef.current;
      const target = controlsRef.current.target;
      const dir = new THREE.Vector3().subVectors(cam.position, target).normalize();
      const dist = cam.position.distanceTo(target);
      if (dist < 15000) {
        cam.position.addScaledVector(dir, 450);
        controlsRef.current.update();
        scheduleBadgeUpdate(100);
      }
    }
  }, [scheduleBadgeUpdate]);

  // Three.js Scene Setup & Single-Mount Lifecycle (Mounts ONCE on container)
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

    // Schedule debounced badge updates on camera interaction
    controls.addEventListener("change", () => {
      scheduleBadgeUpdate(120);
    });
    controls.addEventListener("end", () => {
      projectCentroidBadges();
    });

    // 5. Central Personal Memory Core Mesh at (0, 0, 0)
    const initialPalette = getActiveDynamicPalette(isLightModeRef.current);
    const coreGeo = new THREE.SphereGeometry(76, 32, 32);
    const coreMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      transparent: true,
      opacity: isLightModeRef.current ? 0.95 : 0.88,
    });
    const coreMesh = new THREE.Mesh(coreGeo, coreMat);
    scene.add(coreMesh);
    coreMeshRef.current = coreMesh;

    // Outer Core Halo
    const haloGeo = new THREE.SphereGeometry(96, 24, 24);
    const haloMat = new THREE.MeshBasicMaterial({
      color: initialPalette.personal.threeColor,
      transparent: true,
      opacity: isLightModeRef.current ? 0.38 : 0.22,
      wireframe: true,
    });
    const coreHalo = new THREE.Mesh(haloGeo, haloMat);
    scene.add(coreHalo);
    coreHaloRef.current = coreHalo;

    // 6. InstancedMesh for Fact Nodes (Capacity 12,000)
    const maxNodes = 12000;
    const sphereGeo = new THREE.SphereGeometry(1, 14, 14);
    const nodeMat = new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: isLightModeRef.current ? 0.96 : 0.90,
    });
    const instancedMesh = new THREE.InstancedMesh(sphereGeo, nodeMat, maxNodes);
    instancedMesh.count = 0;
    instancedMesh.frustumCulled = false;
    scene.add(instancedMesh);
    instancedMeshRef.current = instancedMesh;

    // 7. InstancedMesh for Glow Halo Rings
    const ringGeo = new THREE.RingGeometry(1, 1.4, 16);
    const ringMat = new THREE.MeshBasicMaterial({
      transparent: true,
      opacity: isLightModeRef.current ? 0.45 : 0.38,
      side: THREE.DoubleSide,
    });
    const instancedRing = new THREE.InstancedMesh(ringGeo, ringMat, maxNodes);
    instancedRing.count = 0;
    instancedRing.frustumCulled = false;
    scene.add(instancedRing);
    instancedRingRef.current = instancedRing;

    // 8. LineSegments for Dendrite Filaments & Umbilical Conduits (Capacity 40,000)
    const maxLines = 40000;
    const lineGeo = new THREE.BufferGeometry();
    lineGeo.setAttribute("position", new THREE.BufferAttribute(new Float32Array(maxLines * 6), 3));
    lineGeo.setAttribute("color", new THREE.BufferAttribute(new Float32Array(maxLines * 6), 3));

    const lineMat = new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: isLightModeRef.current ? 0.65 : 0.40,
      blending: isLightModeRef.current ? THREE.NormalBlending : THREE.AdditiveBlending,
      depthWrite: false,
    });
    const lineSegments = new THREE.LineSegments(lineGeo, lineMat);
    lineSegments.frustumCulled = false;
    scene.add(lineSegments);
    lineSegmentsRef.current = lineSegments;

    // If facts are already loaded upon mount, populate them immediately!
    if (factsRef.current && factsRef.current.length > 0) {
      rebuildTopology(factsRef.current, isLightModeRef.current);
    }

    // 9. Continuous 60 FPS Animation Loop
    const tempTargetVec = new THREE.Vector3();
    const tempCamVec = new THREE.Vector3();

    const render = () => {
      animFrameRef.current = requestAnimationFrame(render);

      const time = performance.now() * 0.001;

      // Pulse core
      if (coreMeshRef.current) {
        const pulse = 1 + 0.035 * Math.sin(time * 2.2);
        coreMeshRef.current.scale.set(pulse, pulse, pulse);
      }
      if (coreHaloRef.current) {
        coreHaloRef.current.rotation.y = time * 0.15;
        coreHaloRef.current.rotation.x = time * 0.08;
      }

      // Smooth Camera Fly-To Lerp
      if (flyToTargetRef.current && cameraRef.current && controlsRef.current) {
        const target = flyToTargetRef.current;
        const cam = cameraRef.current;
        const ctrl = controlsRef.current;

        tempTargetVec.set(0, 0, 0);
        tempCamVec.set(target.x, target.y, target.z);

        ctrl.target.lerp(tempTargetVec, 0.08);
        cam.position.lerp(tempCamVec, 0.08);

        if (cam.position.distanceTo(tempCamVec) < 2) {
          flyToTargetRef.current = null;
          scheduleBadgeUpdate(30);
        }
      }

      controls.update();
      renderer.render(scene, camera);
    };

    animFrameRef.current = requestAnimationFrame(render);

    // Initial badge projection
    setTimeout(() => {
      projectCentroidBadges();
    }, 100);

    // Unmount Cleanup
    return () => {
      if (badgeTimerRef.current) clearTimeout(badgeTimerRef.current);
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
      haloGeo.dispose();
      haloMat.dispose();
    };
  }, []); // Run ONCE on mount

  // Dedicated Lightweight Resize Effect (does NOT re-create the scene!)
  useEffect(() => {
    const renderer = rendererRef.current;
    const camera = cameraRef.current;
    if (!renderer || !camera || width === 0 || height === 0) return;

    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
    scheduleBadgeUpdate(100);
  }, [width, height, scheduleBadgeUpdate]);

  return {
    isLightMode,
    clusterBadges,
    gNodesRef,
    cameraRef,
    rendererRef,
    instancedMeshRef,
    coreMeshRef,
    recenter,
    focusCore,
    zoomIn,
    zoomOut,
  };
}
