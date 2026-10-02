import type { ActivityEnvelope, InteractionState } from "@/services/eventsService";
import type { OrbState } from "thinking-orbs";

export type OrbMotionPreset = "silk" | "orbits" | "globe" | "web" | "morph";

export interface ActivityDisplayConfig {
  label: string;
  orbState: OrbState;
  motionPreset: OrbMotionPreset;
  shimmer: boolean;
}

/**
 * Tier-1 Registry: Explicit operation name mapping (activity.name).
 */
export const TIER_1_ACTIVITY_REGISTRY: Record<string, Omit<ActivityDisplayConfig, "shimmer">> = {
  web_search: {
    label: "Searching web",
    orbState: "searching",
    motionPreset: "globe",
  },
  search_memory: {
    label: "Recalling memory",
    orbState: "connecting",
    motionPreset: "web",
  },
  memory: {
    label: "Recalling memory",
    orbState: "connecting",
    motionPreset: "web",
  },
  compaction: {
    label: "Compacting context",
    orbState: "shaping",
    motionPreset: "morph",
  },
};

/**
 * Tier-2 Registry: Operation class fallback (activity.kind).
 */
export const TIER_2_KIND_REGISTRY: Record<string, Omit<ActivityDisplayConfig, "shimmer">> = {
  tool: {
    label: "Working",
    orbState: "working",
    motionPreset: "orbits",
  },
  compaction: {
    label: "Compacting context",
    orbState: "shaping",
    motionPreset: "morph",
  },
};

/**
 * Three-tier cascade resolution function per design-spec.md §8 and ipc-spec.md §4.1:
 * 1. activity.name
 * 2. activity.kind
 * 3. interactionState
 */
export function resolveActivityDisplay(
  state: InteractionState,
  activity: ActivityEnvelope | null | undefined,
  isEngaged: boolean,
  isSleeping: boolean,
  isPaused: boolean
): ActivityDisplayConfig {
  if (state === "Error") {
    return {
      label: "Error",
      orbState: "breathing",
      motionPreset: "silk",
      shimmer: false,
    };
  }

  if (!isEngaged || state === "Idle") {
    return {
      label: "Dormant",
      orbState: "breathing",
      motionPreset: "silk",
      shimmer: false,
    };
  }

  if (isSleeping || state === "Sleeping") {
    return {
      label: "Sleeping",
      orbState: "breathing",
      motionPreset: "silk",
      shimmer: false,
    };
  }

  if (isPaused || state === "Paused") {
    return {
      label: "Paused",
      orbState: "breathing",
      motionPreset: "silk",
      shimmer: false,
    };
  }

  // If in Working state, execute Tier 1 & Tier 2 cascade
  if (state === "Working" && activity) {
    // Tier 1: Match by activity.name
    if (activity.name && TIER_1_ACTIVITY_REGISTRY[activity.name]) {
      const t1 = TIER_1_ACTIVITY_REGISTRY[activity.name];
      return {
        ...t1,
        shimmer: true,
      };
    }

    // Tier 2: Match by activity.kind
    if (activity.kind && TIER_2_KIND_REGISTRY[activity.kind]) {
      const t2 = TIER_2_KIND_REGISTRY[activity.kind];
      return {
        ...t2,
        shimmer: true,
      };
    }

    // Fallback within Working
    return {
      label: "Working",
      orbState: "working",
      motionPreset: "orbits",
      shimmer: true,
    };
  }

  // Tier 3: InteractionState baseline
  switch (state) {
    case "Ready":
      return {
        label: "Ready",
        orbState: "breathing",
        motionPreset: "silk",
        shimmer: true,
      };
    case "Listening":
      return {
        label: "Listening",
        orbState: "listening",
        motionPreset: "silk",
        shimmer: true,
      };
    case "Thinking":
      return {
        label: "Thinking",
        orbState: "solving",
        motionPreset: "silk",
        shimmer: true,
      };
    case "Speaking":
      return {
        label: "Speaking",
        orbState: "composing",
        motionPreset: "silk",
        shimmer: true,
      };
    case "Working":
      return {
        label: "Working",
        orbState: "working",
        motionPreset: "orbits",
        shimmer: true,
      };
    default:
      return {
        label: state,
        orbState: "breathing",
        motionPreset: "silk",
        shimmer: false,
      };
  }
}
