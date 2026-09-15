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
 * Generates a vibrant, harmonic color by applying an angular hue shift to the base accent hue.
 * In light mode, lightness is tuned for rich contrast against pale backgrounds (0.38 - 0.46).
 * In dark mode, lightness is tuned for luminous luminescence against dark backgrounds (0.62 - 0.70).
 */
function createHarmonicColor(
  baseH: number,
  hOffset: number,
  isLight: boolean,
  sOverride?: number,
  lOverride?: number
): THREE.Color {
  const h = ((baseH + hOffset) % 1.0 + 1.0) % 1.0;
  const s = sOverride ?? (isLight ? 0.88 : 0.94);
  const l = lOverride ?? (isLight ? 0.42 : 0.65);
  const col = new THREE.Color();
  col.setHSL(h, s, l);
  return col;
}

/**
 * Resolves the active dynamic graph palette derived directly from the primary accent (--accent).
 * All 6 memory categories revolve harmoniously around the user's chosen accent hue, ensuring
 * that changing the accent dynamically updates the entire graph and all UI legends.
 */
export function getActiveDynamicPalette(isLight: boolean): DynamicGraphPalette {
  // Primary accent is ground truth for center / personal
  const accentColor = getCSSVariableColor("--accent", isLight ? "#0e7490" : "#00dbe9");

  // Extract base accent HSL
  const hsl = { h: 0, s: 0, l: 0 };
  accentColor.getHSL(hsl);

  // If accent is near monochrome (e.g. grayscale), use pleasant default base hue
  const baseH = hsl.s < 0.1 ? 0.55 : hsl.h;

  // Deriving 6 harmonious categories dynamically from accent hue:
  // 1. personal: Exact primary accent (Ground Truth identity core)
  // 2. objective: +32deg analogous shift (Harmonious violet/indigo/blue depending on accent)
  // 3. workdone: +122deg triadic harmony (Vibrant fresh mint/emerald/green)
  // 4. blocker: +180deg complementary opposite (Bold contrast crimson/coral/rose)
  // 5. next_step: +245deg harmonic direction (Luminous amber/gold/orange)
  // 6. pitfall: +310deg analogous warm balance (Warm amber-coral/canary)
  const personalColor = accentColor;
  const objectiveColor = createHarmonicColor(baseH, 0.09, isLight, isLight ? 0.88 : 0.94, isLight ? 0.44 : 0.52);
  const workdoneColor = createHarmonicColor(baseH, 0.34, isLight, isLight ? 0.86 : 0.92, isLight ? 0.39 : 0.48);
  const blockerColor = createHarmonicColor(baseH, 0.50, isLight, isLight ? 0.92 : 0.96, isLight ? 0.45 : 0.52);
  const nextStepColor = createHarmonicColor(baseH, 0.68, isLight, isLight ? 0.90 : 0.95, isLight ? 0.42 : 0.50);
  const pitfallColor = createHarmonicColor(baseH, 0.86, isLight, isLight ? 0.92 : 0.96, isLight ? 0.46 : 0.52);

  const glowAlpha = isLight ? 0.35 : 0.45;

  const toEntry = (col: THREE.Color, desc: string): PaletteEntry => {
    const hex = colorToHex(col);
    return {
      main: hex,
      glow: `rgba(${Math.round(col.r * 255)}, ${Math.round(col.g * 255)}, ${Math.round(col.b * 255)}, ${glowAlpha})`,
      text: hex,
      desc,
      threeColor: col,
    };
  };

  return {
    personal: toEntry(personalColor, "Identity facts, core values, user preferences, and foundational profile."),
    objective: toEntry(objectiveColor, "Active operational goals, session intents, and target deliverables."),
    workdone: toEntry(workdoneColor, "Completed milestones, accomplishments, verified tasks, and progress."),
    blocker: toEntry(blockerColor, "Active blockers, missing dependencies, compilation/runtime errors."),
    next_step: toEntry(nextStepColor, "Immediate upcoming actions, planned follow-ups, and roadmap tasks."),
    pitfall: toEntry(pitfallColor, "Edge cases, lessons learned, gotchas, and architectural traps."),
  };
}
