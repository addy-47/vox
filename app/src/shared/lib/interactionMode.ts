/**
 * Normalized interaction mode types and helper functions.
 * Resolves the duality between settings/IPC lowercase ("passive" | "ptt")
 * and VoiceSessionContext uppercase ("PASSIVE" | "PTT").
 */

export type InteractionModeUpper = "PASSIVE" | "PTT";
export type InteractionModeLower = "passive" | "ptt";

export type AnyInteractionMode = InteractionModeUpper | InteractionModeLower | string;

/**
 * Normalizes any interaction mode representation into canonical uppercase "PASSIVE" | "PTT".
 * Defaults safely to "PASSIVE".
 */
export function normalizeToInteractionModeUpper(mode?: AnyInteractionMode | null): InteractionModeUpper {
  if (!mode) return "PASSIVE";
  const upper = String(mode).trim().toUpperCase();
  return upper === "PTT" ? "PTT" : "PASSIVE";
}

/**
 * Normalizes any interaction mode representation into canonical lowercase "passive" | "ptt".
 * Defaults safely to "passive".
 */
export function normalizeToInteractionModeLower(mode?: AnyInteractionMode | null): InteractionModeLower {
  if (!mode) return "passive";
  const lower = String(mode).trim().toLowerCase();
  return lower === "ptt" ? "ptt" : "passive";
}
