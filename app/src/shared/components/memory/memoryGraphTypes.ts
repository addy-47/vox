import {
  User,
  Target,
  CheckCircle2,
  AlertTriangle,
  ArrowRightCircle,
  HelpCircle,
} from "lucide-react";
import { FactRecord } from "@/services/memoryService";

export type MemoryCategory = "personal" | "objective" | "workdone" | "blocker" | "next_step" | "pitfall";

export interface GNode {
  id: string;
  label: string;
  compactId: string;
  collection: MemoryCategory;
  status: "active" | "inactive";
  factRecord: FactRecord;
  color: string;
  degree: number;
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  isCore?: boolean;
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

export interface ClusterBadgeData {
  collection: string;
  graphX: number;
  graphY: number;
  graphZ: number;
  screenX: number;
  screenY: number;
  factCount: number;
  color: string;
  desc: string;
}

export interface MemoryGraphRef {
  recenter: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
  focusCore: () => void;
}


export const DARK_COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = {
  personal: {
    main: "#00dbe9",
    glow: "rgba(0, 219, 233, 0.45)",
    text: "#00dbe9",
    desc: "Identity facts, core values, user preferences, and foundational profile.",
  },
  objective: {
    main: "#a78bfa",
    glow: "rgba(167, 139, 250, 0.45)",
    text: "#a78bfa",
    desc: "Active operational goals, session intents, and target deliverables.",
  },
  workdone: {
    main: "#34d399",
    glow: "rgba(52, 211, 153, 0.45)",
    text: "#34d399",
    desc: "Completed milestones, accomplishments, verified tasks, and progress.",
  },
  blocker: {
    main: "#f43f5e",
    glow: "rgba(244, 63, 94, 0.45)",
    text: "#f43f5e",
    desc: "Active blockers, missing dependencies, compilation/runtime errors.",
  },
  next_step: {
    main: "#f59e0b",
    glow: "rgba(245, 158, 11, 0.45)",
    text: "#f59e0b",
    desc: "Immediate upcoming actions, planned follow-ups, and roadmap tasks.",
  },
  pitfall: {
    main: "#facc15",
    glow: "rgba(250, 204, 21, 0.45)",
    text: "#facc15",
    desc: "Edge cases, lessons learned, gotchas, and architectural traps.",
  },
};

export const LIGHT_COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = {
  personal: {
    main: "#0891b2",
    glow: "rgba(8, 145, 178, 0.4)",
    text: "#0891b2",
    desc: "Identity facts, core values, user preferences, and foundational profile.",
  },
  objective: {
    main: "#7c3aed",
    glow: "rgba(124, 58, 237, 0.4)",
    text: "#7c3aed",
    desc: "Active operational goals, session intents, and target deliverables.",
  },
  workdone: {
    main: "#059669",
    glow: "rgba(5, 150, 105, 0.4)",
    text: "#059669",
    desc: "Completed milestones, accomplishments, verified tasks, and progress.",
  },
  blocker: {
    main: "#e11d48",
    glow: "rgba(225, 29, 72, 0.4)",
    text: "#e11d48",
    desc: "Active blockers, missing dependencies, compilation/runtime errors.",
  },
  next_step: {
    main: "#d97706",
    glow: "rgba(217, 119, 6, 0.4)",
    text: "#d97706",
    desc: "Immediate upcoming actions, planned follow-ups, and roadmap tasks.",
  },
  pitfall: {
    main: "#ca8a04",
    glow: "rgba(202, 138, 4, 0.4)",
    text: "#ca8a04",
    desc: "Edge cases, lessons learned, gotchas, and architectural traps.",
  },
};

import { getActiveDynamicPalette } from "./dynamicGraphPalette";
export * from "./dynamicGraphPalette";

export function getThemeCollectionColors(isLight: boolean) {
  return getActiveDynamicPalette(isLight);
}

export function getCollectionColor(collection: string, _isInactive = false, isLight = false) {
  const palette = getActiveDynamicPalette(isLight);
  return palette[collection as MemoryCategory] ?? palette.objective;
}


export function getCollectionIcon(collection: string) {
  switch (collection) {
    case "personal":
      return User;
    case "objective":
      return Target;
    case "workdone":
      return CheckCircle2;
    case "blocker":
      return AlertTriangle;
    case "next_step":
      return ArrowRightCircle;
    case "pitfall":
      return HelpCircle;
    default:
      return Target;
  }
}
