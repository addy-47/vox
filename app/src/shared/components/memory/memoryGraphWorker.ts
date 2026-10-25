import type { ObservationRecord } from "@/services/memoryService";
import type { GNode, GLink, MemoryCategory } from "./memoryGraphTypes";
import type { DynamicGraphPalette } from "./dynamicGraphPalette";

export interface SessionAnchorData {
  sId: string;
  count: number;
  x: number;
  y: number;
  z: number;
  color: string;
  timestamp: number;
}

export interface ConduitData {
  p0: { x: number; y: number; z: number };
  p1: { x: number; y: number; z: number };
  col0: string;
  col1: string;
  sessionId: string;
}

export interface GraphWorkerInput {
  facts: ObservationRecord[];
  isLight: boolean;
  dynamicPalette: DynamicGraphPalette;
  coreAccentHex: string;
}

export interface GraphWorkerOutput {
  gNodes: GNode[];
  gLinks: GLink[];
  sessionAnchors: SessionAnchorData[];
  conduits: ConduitData[];
}

interface Vec3 {
  x: number;
  y: number;
  z: number;
}

function normalize(v: Vec3): Vec3 {
  const len = Math.hypot(v.x, v.y, v.z) || 1;
  return { x: v.x / len, y: v.y / len, z: v.z / len };
}

function cross(a: Vec3, b: Vec3): Vec3 {
  return {
    x: a.y * b.z - a.z * b.y,
    y: a.z * b.x - a.x * b.z,
    z: a.x * b.y - a.y * b.x,
  };
}

function add(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}

function scale(v: Vec3, s: number): Vec3 {
  return { x: v.x * s, y: v.y * s, z: v.z * s };
}

self.onmessage = (e: MessageEvent<GraphWorkerInput>) => {
  const { facts: inputFacts, dynamicPalette, coreAccentHex } = e.data;

  const gNodes: GNode[] = [];
  const gLinks: GLink[] = [];
  const sessionAnchors: SessionAnchorData[] = [];
  const conduits: ConduitData[] = [];

  // 1. Partition facts into Identity (Personal) and Session facts
  const identityFacts: ObservationRecord[] = [];
  const sessionMap = new Map<string, ObservationRecord[]>();

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

    const colPalette = dynamicPalette.personal;

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

  // 3. 360-Degree Organic Dendritic Tree Canopy (No silent caps or thresholds)
  const sessionMinTimeMap = new Map<string, number>();
  for (const [key, sessionFacts] of sessionMap.entries()) {
    sessionMinTimeMap.set(
      key,
      sessionFacts.reduce((min, f) => Math.min(min, f.created_at || 0), Infinity)
    );
  }

  const sortedSessionKeys = Array.from(sessionMap.keys()).sort((a, b) => {
    const minA = sessionMinTimeMap.get(a) ?? Infinity;
    const minB = sessionMinTimeMap.get(b) ?? Infinity;
    return minA - minB;
  });

  const nSessions = sortedSessionKeys.length;

  sortedSessionKeys.forEach((sKey, sIdx) => {
    const clusterFacts = sessionMap.get(sKey)!;
    const nCluster = clusterFacts.length;
    const sessionTime = clusterFacts[0]?.created_at || sIdx;

    const azimuth = (2 * Math.PI * sIdx) / phiWeight;
    const elevation = Math.asin(-0.65 + (1.30 * (sIdx + 0.5)) / Math.max(nSessions, 1));
    const rSession = 1100 + 160 * Math.sin(sIdx * 1.8) + 80 * Math.cos(sIdx * 3.1);

    const cosEl = Math.cos(elevation);
    const trunkX = rSession * cosEl * Math.cos(azimuth);
    const trunkY = rSession * Math.sin(elevation) * 0.85;
    const trunkZ = rSession * cosEl * Math.sin(azimuth);
    const sessionAnchor: Vec3 = { x: trunkX, y: trunkY, z: trunkZ };

    const dominantCat = (clusterFacts[0]?.fact_type as MemoryCategory) || "objective";

    sessionAnchors.push({
      sId: sKey,
      count: nCluster,
      x: trunkX,
      y: trunkY,
      z: trunkZ,
      color: coreAccentHex,
      timestamp: sessionTime,
    });

    const dir = normalize(sessionAnchor);
    const worldUp: Vec3 = Math.abs(dir.y) < 0.85 ? { x: 0, y: 1, z: 0 } : { x: 1, y: 0, z: 0 };
    const lateral = normalize(cross(dir, worldUp));
    const normal = normalize(cross(dir, lateral));

    const curlSign = sIdx % 2 === 0 ? 1 : -1;
    const curlMag1 = (60 + (sIdx % 5) * 20) * curlSign;
    const curlMag2 = -(40 + (sIdx % 4) * 25) * curlSign;

    const p0: Vec3 = { x: 0, y: 0, z: 0 };
    const p1: Vec3 = add(
      add(scale(dir, rSession * 0.35), scale(lateral, curlMag1)),
      scale(normal, curlMag2 * 0.5)
    );
    const p2: Vec3 = add(
      add(scale(dir, rSession * 0.70), scale(lateral, curlMag2)),
      scale(normal, curlMag1 * 0.5)
    );
    const p3: Vec3 = { ...sessionAnchor };

    const numTrunkSegments = 7;
    let prevPoint: Vec3 = { ...p0 };
    for (let seg = 1; seg <= numTrunkSegments; seg++) {
      const t = seg / numTrunkSegments;
      const u = 1 - t;
      const pt: Vec3 = add(
        add(scale(p0, u * u * u), scale(p1, 3 * u * u * t)),
        add(scale(p2, 3 * u * t * t), scale(p3, t * t * t))
      );

      conduits.push({
        p0: prevPoint,
        p1: pt,
        col0: coreAccentHex,
        col1: coreAccentHex,
        sessionId: sKey,
      });
      prevPoint = pt;
    }

    const anchorNodeIdx = gNodes.length;
    gNodes.push({
      id: `anchor_${sKey}`,
      label: `Session ${sKey}`,
      compactId: `S-${sKey.slice(-4)}`,
      collection: dominantCat,
      status: "active",
      factRecord: clusterFacts[0],
      sessionId: sKey,
      isSessionAnchor: true,
      color: coreAccentHex,
      degree: nCluster,
      x: trunkX,
      y: trunkY,
      z: trunkZ,
      vx: 0,
      vy: 0,
      vz: 0,
    });

    const clusterNodeIndices: number[] = [];

    clusterFacts.forEach((fact, fIdx) => {
      const cat = (fact.fact_type as MemoryCategory) || "objective";
      const palette = dynamicPalette[cat] ?? dynamicPalette.objective;

      const rotAngle = (2 * Math.PI * fIdx) / phiWeight;
      const progress = (fIdx + 1) / Math.max(nCluster, 1);
      const divergence = 0.28 + 0.58 * Math.sqrt(progress);
      const branchDist = 80 + 260 * Math.pow(progress, 0.58);

      const rOff = Math.sin(divergence) * branchDist;
      const aOff = Math.cos(divergence) * branchDist;

      const posX = trunkX + aOff * dir.x + rOff * (Math.cos(rotAngle) * lateral.x + Math.sin(rotAngle) * normal.x);
      const posY = trunkY + aOff * dir.y + rOff * (Math.cos(rotAngle) * lateral.y + Math.sin(rotAngle) * normal.y);
      const posZ = trunkZ + aOff * dir.z + rOff * (Math.cos(rotAngle) * lateral.z + Math.sin(rotAngle) * normal.z);

      const nodeIdx = gNodes.length;
      clusterNodeIndices.push(nodeIdx);

      gNodes.push({
        id: fact.id,
        label: fact.text,
        compactId: fact.id.slice(0, 6),
        collection: cat,
        status: "active",
        factRecord: fact,
        sessionId: sKey,
        color: palette.main,
        degree: 1,
        x: posX,
        y: posY,
        z: posZ,
        vx: 0,
        vy: 0,
        vz: 0,
      });

      let linkSourceIndex = anchorNodeIdx;
      let linkFromId = `anchor_${sKey}`;

      if (fIdx >= 3 && clusterNodeIndices.length > 2) {
        const parentTierIdx = Math.floor((fIdx - 1) / 2.2);
        if (parentTierIdx < clusterNodeIndices.length - 1) {
          linkSourceIndex = clusterNodeIndices[parentTierIdx];
          linkFromId = clusterFacts[parentTierIdx].id;
        }
      }

      gLinks.push({
        id: `link_branch_${fact.id}`,
        sourceIndex: linkSourceIndex,
        targetIndex: nodeIdx,
        fromId: linkFromId,
        toId: fact.id,
        relation: cat.toUpperCase(),
        color: palette.main,
        isDashed: false,
      });
    });
  });

  const output: GraphWorkerOutput = {
    gNodes,
    gLinks,
    sessionAnchors,
    conduits,
  };

  self.postMessage(output);
};
