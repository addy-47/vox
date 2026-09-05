import { Heart, User, Compass, BookOpen, Box, ShieldAlert, Archive } from "lucide-react";
import { MemoryNodeTopology } from "@/services/memoryService";

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

export interface CrossRelation {
  targetCollection: string;
  relation: string;
  count: number;
}

export interface ClusterBadgeData {
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

export interface MemoryGraphRef {
  recenter: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
}

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
    main: "#0369a1",
    glow: "rgba(3, 105, 161, 0.45)",
    text: "#0369a1",
    desc: "Core identity facts, user name, preferences, and foundational attributes.",
  },
  Profile: {
    main: "#047857",
    glow: "rgba(4, 120, 87, 0.45)",
    text: "#047857",
    desc: "Personal background, career history, contacts, and personal metadata.",
  },
  Directives: {
    main: "#6d28d9",
    glow: "rgba(109, 40, 217, 0.45)",
    text: "#6d28d9",
    desc: "Active operational rules, user instructions, system prompts, and priorities.",
  },
  Narrative: {
    main: "#be185d",
    glow: "rgba(190, 24, 93, 0.45)",
    text: "#be185d",
    desc: "Temporal story facts, conversation context, historical events, and session logs.",
  },
  Entities: {
    main: "#b45309",
    glow: "rgba(180, 83, 9, 0.45)",
    text: "#b45309",
    desc: "Projects, codebase modules, tools, software stack, and external references.",
  },
  Constraints: {
    main: "#be123c",
    glow: "rgba(190, 18, 60, 0.45)",
    text: "#be123c",
    desc: "Hard system constraints, hardware limits, security bounds, and forbidden rules.",
  },
  Inactive: {
    main: "#334155",
    glow: "rgba(51, 65, 85, 0.35)",
    text: "#334155",
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
  if (norm.includes("SUPPORT")) return { color: isLight ? "#047857" : "#34d399", isDashed: false };
  if (norm.includes("SUPERSEDE")) return { color: isLight ? "#0369a1" : "#38bdf8", isDashed: false };
  if (norm.includes("SHAPE")) return { color: isLight ? "#6d28d9" : "#a78bfa", isDashed: false };
  if (norm.includes("DEPEND")) return { color: isLight ? "#b45309" : "#facc15", isDashed: false };
  if (norm.includes("CONFLICT") || norm.includes("RESTRICT")) return { color: isLight ? "#be123c" : "#ef4444", isDashed: true };
  return { color: isLight ? "#475569" : "#64748b", isDashed: true };
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
