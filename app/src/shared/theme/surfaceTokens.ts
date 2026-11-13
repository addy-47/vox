import { hexToRgb } from "@/shared/lib/utils";
import {
  PRESET_THEMES,
  DEFAULT_DARK_ACCENT,
  DEFAULT_LIGHT_ACCENT,
} from "./themePresets";

export interface DerivedSurfaceTokens {
  background: string;
  foreground: string;
  foregroundMuted: string;
  card: string;
  field: string;
  sidebar: string;
  border: string;
  accent: string;
  accentComp: string;
  glassSurface: string;
  glassDeep: string;
  ambStop0: string;
  ambStop1: string;
  ambStop2: string;
}

export function hsl2rgb(h: number, s: number, l: number): [number, number, number] {
  if (s === 0) {
    const v = Math.round(l * 255);
    return [v, v, v];
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const hue2rgb = (t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return [
    Math.round(hue2rgb(h + 1 / 3) * 255),
    Math.round(hue2rgb(h) * 255),
    Math.round(hue2rgb(h - 1 / 3) * 255),
  ];
}

export function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (max + min - d) : d / (max + min);
  let h: number;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;
  return [h, s, l];
}

/**
 * Returns the CSS RGB triplet string for the complementary/secondary accent.
 */
export function complementaryTriplet(rgbTriplet: string): string {
  const parts = rgbTriplet.split(",").map((s) => parseInt(s.trim(), 10));
  if (parts.length !== 3 || parts.some(isNaN)) return "168, 85, 247";
  const [r, g, b] = parts;
  const [h, s, l] = rgbToHsl(r / 255, g / 255, b / 255);

  const hDeg = h * 360;
  let targetH: number;
  if (hDeg >= 290 && hDeg <= 355) {
    targetH = 271 / 360;
  } else if (hDeg >= 160 && hDeg <= 230) {
    targetH = 271 / 360;
  } else if (hDeg > 230 && hDeg < 290) {
    targetH = 319 / 360;
  } else {
    targetH = ((h + 0.22) % 1 + 1) % 1;
  }

  const sComp = Math.min(Math.max(s, 0.80), 0.92);
  const lComp = Math.min(Math.max(l, 0.58), 0.66);
  const [cr, cg, cb] = hsl2rgb(targetH, sComp, lComp);
  return `${cr}, ${cg}, ${cb}`;
}

export function applyComplementaryToken(accentTriplet: string): void {
  document.documentElement.style.setProperty(
    "--accent-complementary",
    complementaryTriplet(accentTriplet)
  );
}

export function deriveSurfaceTokens(bgHex: string, isLight: boolean): DerivedSurfaceTokens {
  const normHex = bgHex.trim().toUpperCase();
  for (const preset of PRESET_THEMES) {
    const target = isLight ? preset.light : preset.dark;
    if (target.bg.toUpperCase() === normHex) {
      const bgTrip = hexToRgb(target.bg);
      const cardTrip = hexToRgb(target.card);
      const fieldTrip = hexToRgb(target.field);
      const fgTrip = hexToRgb(target.fg);
      const fgMutedTrip = hexToRgb(target.fgMuted);
      const [r, g, b] = bgTrip.split(",").map((s) => parseInt(s.trim(), 10));
      const [h, s, l] = rgbToHsl(r / 255, g / 255, b / 255);
      const [s0r, s0g, s0b] = hsl2rgb(h, s, isLight ? Math.min(1, l + 0.02) : Math.min(1, l + 0.03));
      const [s2r, s2g, s2b] = hsl2rgb(h, s, Math.max(0, l - 0.03));
      return {
        background: bgTrip,
        foreground: fgTrip,
        foregroundMuted: fgMutedTrip,
        card: cardTrip,
        field: fieldTrip,
        sidebar: fieldTrip,
        border: target.border,
        accent: target.accent,
        accentComp: target.accentComp,
        glassSurface: cardTrip,
        glassDeep: fieldTrip,
        ambStop0: `${s0r}, ${s0g}, ${s0b}`,
        ambStop1: bgTrip,
        ambStop2: `${s2r}, ${s2g}, ${s2b}`,
      };
    }
  }

  const parts = hexToRgb(bgHex).split(",").map((s) => parseInt(s.trim(), 10));
  const [r, g, b] = parts.length === 3 && !parts.some(isNaN) ? parts : [19, 19, 21];
  const [h, s, l] = rgbToHsl(r / 255, g / 255, b / 255);

  if (!isLight) {
    const cardL = Math.min(1, l + 0.05);
    const [cr, cg, cb] = hsl2rgb(h, Math.max(0, s - 0.02), cardL);
    const fieldL = Math.max(0, l - 0.015);
    const [fr, fg, fb] = hsl2rgb(h, s, fieldL);
    const sideL = Math.max(0, l + 0.02);
    const [sr, sg, sb] = hsl2rgb(h, s, sideL);
    const [s0r, s0g, s0b] = hsl2rgb(h, s, Math.min(1, l + 0.03));
    const [s2r, s2g, s2b] = hsl2rgb(h, s, Math.max(0, l - 0.03));

    return {
      background: `${r}, ${g}, ${b}`,
      foreground: "236, 234, 230",
      foregroundMuted: "156, 153, 148",
      card: `${cr}, ${cg}, ${cb}`,
      field: `${fr}, ${fg}, ${fb}`,
      sidebar: `${sr}, ${sg}, ${sb}`,
      border: "255, 255, 255",
      accent: DEFAULT_DARK_ACCENT,
      accentComp: "168, 85, 247",
      glassSurface: `${cr}, ${cg}, ${cb}`,
      glassDeep: `${fr}, ${fg}, ${fb}`,
      ambStop0: `${s0r}, ${s0g}, ${s0b}`,
      ambStop1: `${r}, ${g}, ${b}`,
      ambStop2: `${s2r}, ${s2g}, ${s2b}`,
    };
  } else {
    const cardTrip = "255, 255, 255";
    const fieldL = Math.max(0, l - 0.03);
    const [fr, fg, fb] = hsl2rgb(h, s, fieldL);
    const sideL = Math.max(0, l - 0.02);
    const [sr, sg, sb] = hsl2rgb(h, s, sideL);
    const [s0r, s0g, s0b] = hsl2rgb(h, s, Math.min(1, l + 0.02));
    const [s2r, s2g, s2b] = hsl2rgb(h, s, Math.max(0, l - 0.03));

    return {
      background: `${r}, ${g}, ${b}`,
      foreground: "28, 26, 24",
      foregroundMuted: "102, 98, 92",
      card: cardTrip,
      field: `${fr}, ${fg}, ${fb}`,
      sidebar: `${sr}, ${sg}, ${sb}`,
      border: "0, 0, 0",
      accent: DEFAULT_LIGHT_ACCENT,
      accentComp: "124, 58, 237",
      glassSurface: cardTrip,
      glassDeep: `${fr}, ${fg}, ${fb}`,
      ambStop0: `${s0r}, ${s0g}, ${s0b}`,
      ambStop1: `${r}, ${g}, ${b}`,
      ambStop2: `${s2r}, ${s2g}, ${s2b}`,
    };
  }
}
