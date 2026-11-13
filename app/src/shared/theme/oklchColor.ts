import { oklch, formatHex } from "culori";

export interface OklchColor {
  l: number;
  c: number;
  h: number;
}

/**
 * Converts a hex color string to OKLCH coordinates.
 * Provides fallback defaults when components (e.g. hue for pure black/white) are undefined.
 */
export function hexToOklch(hex: string, fallbackHue = 200): OklchColor {
  try {
    const col = oklch(hex);
    if (!col) return { l: 0.8, c: 0.15, h: fallbackHue };
    return {
      l: typeof col.l === "number" && !isNaN(col.l) ? col.l : 0.8,
      c: typeof col.c === "number" && !isNaN(col.c) ? col.c : 0.15,
      h: typeof col.h === "number" && !isNaN(col.h) ? col.h : fallbackHue,
    };
  } catch {
    return { l: 0.8, c: 0.15, h: fallbackHue };
  }
}

/**
 * Converts OKLCH coordinates to an uppercase hex color string.
 */
export function oklchToHex(l: number, c: number, h: number): string {
  try {
    const clampedL = Math.max(0, Math.min(1, l));
    const clampedC = Math.max(0, Math.min(0.35, c));
    const normalizedH = ((h % 360) + 360) % 360;
    const col = oklch({ mode: "oklch", l: clampedL, c: clampedC, h: normalizedH });
    return (formatHex(col) || "#000000").toUpperCase();
  } catch {
    return "#000000";
  }
}

/**
 * Precomputed OKLCH conic-gradient for the 360° Accent Hue Ring.
 */
export const ACCENT_HUE_CONIC =
  "conic-gradient(from 0deg, #fd7eaa 0deg, #ff826f 30deg, #f6922e 60deg, #d5a800 90deg, #a2bc32 120deg, #55c975 150deg, #00ceb2 180deg, #00c7e5 210deg, #2cb9ff 240deg, #87a6ff 270deg, #be94ff 300deg, #e586de 330deg, #fd7eaa 360deg)";

/**
 * Precomputed OKLCH conic-gradient for the Dark Mode Background Tint Ring.
 */
export const DARK_BG_CONIC =
  "conic-gradient(from 0deg, #2d0f1a 0deg, #2e100c 30deg, #2b1401 60deg, #241900 90deg, #181e01 120deg, #06210d 150deg, #00221b 180deg, #002027 210deg, #001d2f 240deg, #121932 270deg, #1e142e 300deg, #271125 330deg, #2d0f1a 360deg)";

/**
 * Precomputed OKLCH conic-gradient for the Light Mode Background Tint Ring.
 */
export const LIGHT_BG_CONIC =
  "conic-gradient(from 0deg, #fee3ea 0deg, #ffe4df 30deg, #fbe7d8 60deg, #f3ebd5 90deg, #e8efd8 120deg, #def1e1 150deg, #d7f2ec 180deg, #d5f1f6 210deg, #daeefe 240deg, #e3ebff 270deg, #eee7fd 300deg, #f7e4f5 330deg, #fee3ea 360deg)";
