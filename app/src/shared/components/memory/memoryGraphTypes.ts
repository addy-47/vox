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
  flyToSession: (sessionId: string) => void;
  flyToNode: (factId: string) => void;
}



import { getActiveDynamicPalette } from "./dynamicGraphPalette";
export * from "./dynamicGraphPalette";

export const DARK_COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = new Proxy(
  {},
  {
    get: (_target, prop: string) => {
      const palette = getActiveDynamicPalette(false);
      return palette[prop as MemoryCategory] ?? palette.personal;
    },
  }
);

export const LIGHT_COLLECTION_COLORS: Record<string, { main: string; glow: string; text: string; desc: string }> = new Proxy(
  {},
  {
    get: (_target, prop: string) => {
      const palette = getActiveDynamicPalette(true);
      return palette[prop as MemoryCategory] ?? palette.personal;
    },
  }
);

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
