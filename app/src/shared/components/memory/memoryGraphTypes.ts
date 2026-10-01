import {
  User,
  Target,
  CheckCircle2,
  AlertTriangle,
  ArrowRightCircle,
  HelpCircle,
} from "lucide-react";
import { ObservationRecord } from "@/services/memoryService";

export type MemoryCategory = "personal" | "objective" | "workdone" | "blocker" | "next_step" | "pitfall";

const MEMORY_CATEGORIES: readonly MemoryCategory[] = [
  "personal",
  "objective",
  "workdone",
  "blocker",
  "next_step",
  "pitfall",
];

/** Validates a stored fact type into a palette category. Unknown values yield
 * undefined so call sites fall back instead of indexing with a lie. */
export function toMemoryCategory(value: string): MemoryCategory | undefined {
  return MEMORY_CATEGORIES.find((c) => c === value);
}

export interface GNode {
  id: string;
  label: string;
  compactId: string;
  collection: MemoryCategory;
  status: "active" | "inactive";
  factRecord: ObservationRecord;
  color: string;
  degree: number;
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  isCore?: boolean;
  sessionId?: string | null;
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


export function getCollectionColor(collection: string, _isInactive = false, isLight = false) {
  const palette = getActiveDynamicPalette(isLight);
  const kind = toMemoryCategory(collection);
  return (kind !== undefined ? palette[kind] : undefined) ?? palette.objective;
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
