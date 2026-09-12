import * as THREE from "three";
import { MemoryCategory } from "./memoryGraphTypes";

export interface PaletteEntry {
  main: string;
  glow: string;
  text: string;
  desc: string;
  threeColor: THREE.Color;
}

export type DynamicGraphPalette = Record<MemoryCategory, PaletteEntry>;

/**
 * Parses CSS variable rgb(r, g, b) or 'r, g, b' into a THREE.Color.
 */
export function getCSSVariableColor(varName: string, fallbackHex: string): THREE.Color {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return new THREE.Color(fallbackHex);
  }
  const val = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  if (!val) return new THREE.Color(fallbackHex);

  const parts = val.split(",").map((s) => parseInt(s.trim(), 10));
  if (parts.length === 3 && !parts.some(isNaN)) {
    return new THREE.Color(parts[0] / 255, parts[1] / 255, parts[2] / 255);
  }
  try {
    return new THREE.Color(val);
  } catch {
    return new THREE.Color(fallbackHex);
  }
}

/**
 * Converts a THREE.Color to hex string '#rrggbb'.
 */
export function colorToHex(col: THREE.Color): string {
  return `#${col.getHexString()}`;
}

/**
 * Resolves the active dynamic graph palette derived directly from the primary accent (--accent)
 * and semantic CSS variables, ensuring the central core always matches primary accent and
 * all hues automatically adjust for light or dark mode.
 */
export function getActiveDynamicPalette(isLight: boolean): DynamicGraphPalette {
  // Primary accent is ground truth for center / personal
  const accentColor = getCSSVariableColor("--accent", isLight ? "#0e7490" : "#00dbe9");
  const violetColor = getCSSVariableColor("--violet", isLight ? "#6d28d9" : "#a78bfa");
  const successColor = getCSSVariableColor("--success", isLight ? "#047857" : "#34d399");
  const dangerColor = getCSSVariableColor("--danger", isLight ? "#be123c" : "#f43f5e");
  const warnSoftColor = getCSSVariableColor("--warn-soft", isLight ? "#d97706" : "#f59e0b");
  const warningColor = getCSSVariableColor("--warning", isLight ? "#ca8a04" : "#facc15");

  const accentHex = colorToHex(accentColor);
  const violetHex = colorToHex(violetColor);
  const successHex = colorToHex(successColor);
  const dangerHex = colorToHex(dangerColor);
  const warnSoftHex = colorToHex(warnSoftColor);
  const warningHex = colorToHex(warningColor);

  const glowAlpha = isLight ? 0.35 : 0.45;

  return {
    personal: {
      main: accentHex,
      glow: `rgba(${Math.round(accentColor.r * 255)}, ${Math.round(accentColor.g * 255)}, ${Math.round(accentColor.b * 255)}, ${glowAlpha})`,
      text: accentHex,
      desc: "Identity facts, core values, user preferences, and foundational profile.",
      threeColor: accentColor,
    },
    objective: {
      main: violetHex,
      glow: `rgba(${Math.round(violetColor.r * 255)}, ${Math.round(violetColor.g * 255)}, ${Math.round(violetColor.b * 255)}, ${glowAlpha})`,
      text: violetHex,
      desc: "Active operational goals, session intents, and target deliverables.",
      threeColor: violetColor,
    },
    workdone: {
      main: successHex,
      glow: `rgba(${Math.round(successColor.r * 255)}, ${Math.round(successColor.g * 255)}, ${Math.round(successColor.b * 255)}, ${glowAlpha})`,
      text: successHex,
      desc: "Completed milestones, accomplishments, verified tasks, and progress.",
      threeColor: successColor,
    },
    blocker: {
      main: dangerHex,
      glow: `rgba(${Math.round(dangerColor.r * 255)}, ${Math.round(dangerColor.g * 255)}, ${Math.round(dangerColor.b * 255)}, ${glowAlpha})`,
      text: dangerHex,
      desc: "Active blockers, missing dependencies, compilation/runtime errors.",
      threeColor: dangerColor,
    },
    next_step: {
      main: warnSoftHex,
      glow: `rgba(${Math.round(warnSoftColor.r * 255)}, ${Math.round(warnSoftColor.g * 255)}, ${Math.round(warnSoftColor.b * 255)}, ${glowAlpha})`,
      text: warnSoftHex,
      desc: "Immediate upcoming actions, planned follow-ups, and roadmap tasks.",
      threeColor: warnSoftColor,
    },
    pitfall: {
      main: warningHex,
      glow: `rgba(${Math.round(warningColor.r * 255)}, ${Math.round(warningColor.g * 255)}, ${Math.round(warningColor.b * 255)}, ${glowAlpha})`,
      text: warningHex,
      desc: "Edge cases, lessons learned, gotchas, and architectural traps.",
      threeColor: warningColor,
    },
  };
}
