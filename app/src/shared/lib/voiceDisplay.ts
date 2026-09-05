import { type InteractionState } from "@/services/eventsService";

export type AmbientMood =
  | "Dormant"
  | "Idle"
  | "Ready"
  | "Listening"
  | "Thinking"
  | "Speaking"
  | "Paused"
  | "Error";

/**
 * Maps current interaction state and engagement flag into the ambient mood enum
 * used by orb and ambient lighting shaders.
 */
export function toMood(state: InteractionState, isEngaged: boolean): AmbientMood {
  if (!isEngaged) return "Dormant";
  switch (state) {
    case "Idle":
      return "Idle";
    case "Ready":
      return "Ready";
    case "Listening":
      return "Listening";
    case "Thinking":
      return "Thinking";
    case "Speaking":
      return "Speaking";
    case "Paused":
      return "Paused";
    case "Error":
      return "Error";
    default:
      return "Dormant";
  }
}

/**
 * Derives the user-facing status label displayed on the HUD/home status cluster.
 */
export function toStatusLabel(
  state: InteractionState,
  engaged: boolean,
  sleeping: boolean,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  isPaused: boolean
): string {
  if (state === "Error") return "Error";
  if (!engaged || state === "Idle") return "Dormant";
  if (isPaused || state === "Paused" || sleeping) return "Paused";
  if (ptt === "RECORDING") return "Recording";
  if (ptt === "PROCESSING") return "Processing";
  switch (state) {
    case "Ready":
      return "Ready";
    case "Listening":
      return "Listening";
    case "Thinking":
      return "Thinking";
    case "Speaking":
      return "Speaking";
    default:
      return "Ready";
  }
}

/**
 * Computes whether the status indicator dot should be pulsing/active.
 */
export function isDotActive(
  engaged: boolean,
  state: InteractionState,
  ptt: "IDLE" | "RECORDING" | "PROCESSING",
  sleeping: boolean
): boolean {
  if (!engaged || sleeping || state === "Idle" || state === "Paused" || state === "Error") return false;
  if (ptt === "RECORDING" || ptt === "PROCESSING") return true;
  return state === "Listening" || state === "Thinking" || state === "Speaking";
}
