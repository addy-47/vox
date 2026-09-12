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
  const animFrameRef = useRef<number | null>(null);
  const isRenderingRef = useRef(false);
  const wakeRenderLoopRef = useRef<() => void>(() => {});
  const userHasNavigatedCameraRef = useRef(false);
  const flyToTargetRef = useRef<{ x: number; y: number; z: number } | null>(null);
  const lastBadgeUpdateRef = useRef(0);

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

  // Detect dark / light mode
  useEffect(() => {
    const updateTheme = () => {
      setIsLightMode(document.documentElement.getAttribute("data-theme") === "light");
    };
    updateTheme();
    const observer = new MutationObserver(updateTheme);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "class"] });
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
    if (!instancedMesh || !instancedRing) return;

    const dummy = dummyObjRef.current;
    const color = colorObjRef.current;
    const selFactId = selectedFactIdRef.current;
    const sq = searchQueryRef.current.trim().toLowerCase();
    const hasSearch = sq.length > 0;

    let visibleCount = 0;

    gNodes.forEach((node, i) => {
      const visible = isNodeVisible(node);
      const matchesSearch = !hasSearch || isNodeMatchingSearch(node);
      const isSelected = selFactId === node.id;
      const palette = getCollectionColor(node.collection, false, isLightMode);

      let radius: number;
      let colHex: string;

      if (!visible) {
        radius = 0.001;
        colHex = palette.main;
      } else if (hasSearch && !matchesSearch) {
        radius = 1.5;
        colHex = isLightMode ? "#94a3b8" : "#283344";
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
      instancedRing.setColorAt(i, color);

      if (visible) visibleCount++;
    });

    instancedMesh.count = gNodes.length;
    instancedRing.count = gNodes.length;

    if (instancedMesh.instanceMatrix) instancedMesh.instanceMatrix.needsUpdate = true;
    if (instancedMesh.instanceColor) instancedMesh.instanceColor.needsUpdate = true;
    if (instancedRing.instanceMatrix) instancedRing.instanceMatrix.needsUpdate = true;
    if (instancedRing.instanceColor) instancedRing.instanceColor.needsUpdate = true;
  }, [isLightMode, isNodeVisible, isNodeMatchingSearch]);

  // Dendritic Constellation Spatial Generator
  useEffect(() => {
    if (!facts || facts.length === 0) return;

    const gNodes: GNode[] = [];
    const gLinks: GLink[] = [];

    // Separate facts
    const identityFacts = facts.filter((f) => f.fact_type === "personal");
    const sessionFacts = facts.filter((f) => f.fact_type !== "personal");

    // Group session facts by session_id
    const sessionGroups = new Map<string, FactRecord[]>();
    for (const f of sessionFacts) {
      const key = f.session_id !== null ? String(f.session_id) : "unsessioned";
      const arr = sessionGroups.get(key) ?? [];
      arr.push(f);
      sessionGroups.set(key, arr);
    }

    // 1. Ring 1 (Identity Shell): R = 340 (Spherical Fibonacci Distribution)
    const rIdentity = 340;
    const nIdent = identityFacts.length;
    const phiWeight = (1 + Math.sqrt(5)) / 2;

    identityFacts.forEach((fact, i) => {
      const theta = (2 * Math.PI * i) / phiWeight;
      const phi = Math.acos(1 - (2 * (i + 0.5)) / nIdent);

      const x = rIdentity * Math.sin(phi) * Math.cos(theta);
      const y = rIdentity * Math.sin(phi) * Math.sin(theta);
      const z = rIdentity * Math.cos(phi);

      const colPalette = getCollectionColor("personal", false, isLightMode);

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
        sourceIndex: -1, // -1 means Core (0, 0, 0)
        targetIndex: gNodes.length - 1,
        fromId: "core",
        toId: fact.id,
        relation: "CORE_IDENTITY",
        color: colPalette.main,
        isDashed: false,
      });
    });

    // 2. Ring 2 (Branching Session Constellations): R = 1100 to 1400
    const rSessionBase = 1150;
    const sessionKeys = Array.from(sessionGroups.keys());
    const nSessions = sessionKeys.length;

    sessionKeys.forEach((sKey, sIdx) => {
      // Fibonacci sphere for session trunk anchors
      const sTheta = (2 * Math.PI * sIdx) / phiWeight;
      const sPhi = Math.acos(1 - (2 * (sIdx + 0.5)) / nSessions);

      const trunkDirX = Math.sin(sPhi) * Math.cos(sTheta);
      const trunkDirY = Math.sin(sPhi) * Math.sin(sTheta);
      const trunkDirZ = Math.cos(sPhi);

      const trunkX = rSessionBase * trunkDirX;
      const trunkY = rSessionBase * trunkDirY;
      const trunkZ = rSessionBase * trunkDirZ;

      const clusterFacts = sessionGroups.get(sKey) ?? [];
      const nCluster = clusterFacts.length;

      // Dendritic conical arbor spreading outwards
      const trunkVec = new THREE.Vector3(trunkDirX, trunkDirY, trunkDirZ).normalize();
      const upVec = Math.abs(trunkVec.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0);
      const rightVec = new THREE.Vector3().crossVectors(trunkVec, upVec).normalize();
      const perpVec = new THREE.Vector3().crossVectors(trunkVec, rightVec).normalize();

      clusterFacts.forEach((fact, fIdx) => {
        const cat = (fact.fact_type as MemoryCategory) || "objective";
        const palette = getCollectionColor(cat, false, isLightMode);

        // Branching arbor cone: angle up to 45 deg, length up to 350
        const coneAngle = 0.15 + 0.55 * Math.sqrt((fIdx + 1) / nCluster);
        const coneRot = fIdx * 2.39996; // Golden angle rotation
        const branchDist = 80 + 260 * Math.pow((fIdx + 1) / nCluster, 0.6);

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
          sourceIndex: -2, // Trunk indicator
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

    // Update LineSegments geometry
    const lineSegments = lineSegmentsRef.current;
    if (lineSegments) {
      const maxLines = gLinks.length;
      const posArray = new Float32Array(maxLines * 6);
      const colArray = new Float32Array(maxLines * 6);

      const colorHelper = new THREE.Color();

      gLinks.forEach((link, idx) => {
        const tgt = gNodes[link.targetIndex];
        if (!tgt) return;

        let srcX = 0, srcY = 0, srcZ = 0;
        if (link.sourceIndex === -1) {
          // Connected to central core
          srcX = 0;
          srcY = 0;
          srcZ = 0;
        } else if (link.sourceIndex === -2) {
          // Connected to session trunk anchor
          const sKey = link.fromId;
          const sIdx = sessionKeys.indexOf(sKey);
          if (sIdx >= 0) {
            const sTheta = (2 * Math.PI * sIdx) / phiWeight;
            const sPhi = Math.acos(1 - (2 * (sIdx + 0.5)) / nSessions);
            srcX = rSessionBase * Math.sin(sPhi) * Math.cos(sTheta);
            srcY = rSessionBase * Math.sin(sPhi) * Math.sin(sTheta);
            srcZ = rSessionBase * Math.cos(sPhi);
          }
        }

        const baseIdx = idx * 6;
        posArray[baseIdx + 0] = srcX;
        posArray[baseIdx + 1] = srcY;
        posArray[baseIdx + 2] = srcZ;
        posArray[baseIdx + 3] = tgt.x;
        posArray[baseIdx + 4] = tgt.y;
        posArray[baseIdx + 5] = tgt.z;

        colorHelper.set(link.color);
        // Fade line near origin
        colArray[baseIdx + 0] = colorHelper.r * 0.3;
        colArray[baseIdx + 1] = colorHelper.g * 0.3;
        colArray[baseIdx + 2] = colorHelper.b * 0.3;
        colArray[baseIdx + 3] = colorHelper.r;
        colArray[baseIdx + 4] = colorHelper.g;
        colArray[baseIdx + 5] = colorHelper.b;
      });

      const lineGeo = lineSegments.geometry;
      lineGeo.setAttribute("position", new THREE.BufferAttribute(posArray, 3));
      lineGeo.setAttribute("color", new THREE.BufferAttribute(colArray, 3));
      lineGeo.attributes.position.needsUpdate = true;
      lineGeo.attributes.color.needsUpdate = true;
    }

    updateWebGLBuffers();
    wakeRenderLoopRef.current();
  }, [facts, isLightMode, updateWebGLBuffers]);

  // Centroid Badges Update
  const updateCentroidBadgesSync = useCallback(() => {
    const now = performance.now();
    if (now - lastBadgeUpdateRef.current < 150) return;
    lastBadgeUpdateRef.current = now;

    const camera = cameraRef.current;
    const renderer = rendererRef.current;
    const gNodes = gNodesRef.current;
    if (!camera || !renderer || gNodes.length === 0) return;

    const sessionAnchors = new Map<string, { count: number; x: number; y: number; z: number }>();
    gNodes.forEach((node) => {
      const sId = node.factRecord.session_id !== null ? String(node.factRecord.session_id) : "core";
      const existing = sessionAnchors.get(sId);
      if (!existing) {
        sessionAnchors.set(sId, { count: 1, x: node.x, y: node.y, z: node.z });
      } else {
        existing.count++;
        existing.x += node.x;
        existing.y += node.y;
        existing.z += node.z;
      }
    });

    const badges: ClusterBadgeData[] = [];
    const tempVec = tempVecRef.current;
    const rect = renderer.domElement.getBoundingClientRect();

    sessionAnchors.forEach((data, sId) => {
      if (data.count < 3 || sId === "core") return;
      const cx = data.x / data.count;
      const cy = data.y / data.count;
      const cz = data.z / data.count;

      tempVec.set(cx, cy, cz);
      tempVec.project(camera);

      // Check if behind camera
      if (tempVec.z > 1) return;

      const screenX = (tempVec.x * 0.5 + 0.5) * rect.width;
      const screenY = (-(tempVec.y * 0.5) + 0.5) * rect.height;

      badges.push({
        collection: `Session #${sId}`,
        graphX: cx,
        graphY: cy,
        graphZ: cz,
        screenX,
        screenY,
        factCount: data.count,
        color: "#a78bfa",
        desc: `${data.count} structured facts synthesized in session #${sId}.`,
      });
    });

    setClusterBadges(badges.slice(0, 16));
  }, []);

  // Smooth Camera Fly-To Lerp when selectedFactId changes
  useEffect(() => {
    if (!selectedFactId) {
      flyToTargetRef.current = null;
      return;
    }
    const targetNode = gNodesRef.current.find((n) => n.id === selectedFactId);
    if (targetNode) {
      flyToTargetRef.current = { x: targetNode.x, y: targetNode.y, z: targetNode.z + 280 };
      wakeRenderLoopRef.current();
    }
  }, [selectedFactId]);

  // Recenter helper
  const recenter = useCallback(() => {
    flyToTargetRef.current = { x: 0, y: 0, z: 2800 };
    wakeRenderLoopRef.current();
  }, []);

  const zoomIn = useCallback(() => {
    if (cameraRef.current) {
      cameraRef.current.position.z = Math.max(cameraRef.current.position.z - 400, 300);
      wakeRenderLoopRef.current();
    }
  }, []);

  const zoomOut = useCallback(() => {
    if (cameraRef.current) {
      cameraRef.current.position.z = Math.min(cameraRef.current.position.z + 400, 7000);
      wakeRenderLoopRef.current();
    }
  }, []);

  // Three.js Scene Setup & Render Loop
  useEffect(() => {
    const container = canvasContainerRef.current;
    if (!container || width === 0 || height === 0) return;

    // 1. Scene
    const scene = new THREE.Scene();
    sceneRef.current = scene;

    // 2. Camera
    const camera = new THREE.PerspectiveCamera(55, width / height, 5, 25000);
    camera.position.set(0, 0, 3100);
    cameraRef.current = camera;

    // 3. Renderer
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setSize(width, height);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.appendChild(renderer.domElement);
    rendererRef.current = renderer;

    // 4. OrbitControls for Full 3D Navigation
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableRotate = true;
    controls.rotateSpeed = 0.65;
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.minDistance = 150;
    controls.maxDistance = 12000;
    controls.zoomSpeed = 0.85;
    controls.panSpeed = 0.8;
    controlsRef.current = controls;

    controls.addEventListener("start", () => {
      userHasNavigatedCameraRef.current = true;
    });

    // 5. Central Personal Memory Core Mesh at (0, 0, 0)
    const coreGeo = new THREE.SphereGeometry(72, 32, 32);
    const coreMat = new THREE.MeshBasicMaterial({
      color: new THREE.Color("#00dbe9"),
      transparent: true,
      opacity: 0.88,
    });
    const coreMesh = new THREE.Mesh(coreGeo, coreMat);
    scene.add(coreMesh);
    coreMeshRef.current = coreMesh;

    // Outer Core Halo
    const haloGeo = new THREE.SphereGeometry(92, 24, 24);
    const haloMat = new THREE.MeshBasicMaterial({
      color: new THREE.Color("#67e8f9"),
      transparent: true,
      opacity: 0.22,
      wireframe: true,
    });
    const coreHalo = new THREE.Mesh(haloGeo, haloMat);
    scene.add(coreHalo);
    coreHaloRef.current = coreHalo;

    // 6. InstancedMesh for Fact Nodes (Capacity 10,000)
    const maxNodes = 10000;
    const sphereGeo = new THREE.SphereGeometry(1, 14, 14);
    const nodeMat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.92 });
    const instancedMesh = new THREE.InstancedMesh(sphereGeo, nodeMat, maxNodes);
    instancedMesh.count = 0;
    instancedMesh.frustumCulled = false;
    scene.add(instancedMesh);
    instancedMeshRef.current = instancedMesh;

    // 7. InstancedMesh for Glow Halo Rings
    const ringGeo = new THREE.RingGeometry(1, 1.4, 16);
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

    // 8. LineSegments for Dendrite Filaments
    const maxLines = 25000;
    const lineGeo = new THREE.BufferGeometry();
    lineGeo.setAttribute("position", new THREE.BufferAttribute(new Float32Array(maxLines * 6), 3));
    lineGeo.setAttribute("color", new THREE.BufferAttribute(new Float32Array(maxLines * 6), 3));

    const lineMat = new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: isLightMode ? 0.5 : 0.35,
    });
    const lineSegments = new THREE.LineSegments(lineGeo, lineMat);
    lineSegments.frustumCulled = false;
    scene.add(lineSegments);
    lineSegmentsRef.current = lineSegments;

    // Render loop
    const render = () => {
      isRenderingRef.current = true;

      // Animate core pulsing and rotation
      const time = performance.now() * 0.001;
      if (coreMeshRef.current) {
        const pulse = 1 + 0.04 * Math.sin(time * 2.5);
        coreMeshRef.current.scale.set(pulse, pulse, pulse);
      }
      if (coreHaloRef.current) {
        coreHaloRef.current.rotation.y = time * 0.2;
        coreHaloRef.current.rotation.x = time * 0.1;
      }

      // Smooth Camera Fly-To Lerp
      let cameraFlying = false;
      if (flyToTargetRef.current && cameraRef.current && controlsRef.current) {
        const target = flyToTargetRef.current;
        const cam = cameraRef.current;
        const ctrl = controlsRef.current;

        ctrl.target.x += (target.x - ctrl.target.x) * 0.1;
        ctrl.target.y += (target.y - ctrl.target.y) * 0.1;
        ctrl.target.z += (0 - ctrl.target.z) * 0.1;

        cam.position.x += (target.x - cam.position.x) * 0.1;
        cam.position.y += (target.y - cam.position.y) * 0.1;
        cam.position.z += (target.z - cam.position.z) * 0.1;

        const dist = cam.position.distanceTo(new THREE.Vector3(target.x, target.y, target.z));
        if (dist > 1.5) {
          cameraFlying = true;
        } else {
          flyToTargetRef.current = null;
        }
      }

      controls.update();
      renderer.render(scene, camera);
      updateCentroidBadgesSync();

      if (cameraFlying || isRenderingRef.current) {
        animFrameRef.current = requestAnimationFrame(render);
      }
    };

    wakeRenderLoopRef.current = () => {
      if (animFrameRef.current === null) {
        animFrameRef.current = requestAnimationFrame(render);
      }
    };

    wakeRenderLoopRef.current();

    // Cleanup & WebGL forceContextLoss
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
      haloGeo.dispose();
      haloMat.dispose();
    };
  }, [width, height, isLightMode, updateCentroidBadgesSync]);

  return {
    isLightMode,
    clusterBadges,
    gNodesRef,
    cameraRef,
    rendererRef,
    instancedMeshRef,
    coreMeshRef,
    recenter,
    zoomIn,
    zoomOut,
    wakeRenderLoop: () => wakeRenderLoopRef.current(),
  };
}
