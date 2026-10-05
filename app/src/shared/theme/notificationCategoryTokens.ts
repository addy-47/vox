/**
 * Notification category tokens (`--notif-<category>`).
 *
 * One RGB triplet per closed notification category (notifications-spec §4.5),
 * derived from the live `--accent` HSL base via harmonic hue rotation — the
 * same approach the memory legend palette uses (dynamicGraphPalette.ts).
 *
 * Invariants (notifications-spec §4.7):
 *  - A category token must never resolve to `--accent` itself: no offset is 0.
 *  - Recomputed on every theme/accent write so the theme flip interpolates
 *    them alongside the rest of the token set.
 *  - Written as a stylesheet rule (not inline props): the flip's end-value
 *    capture drops inline non-accent properties before reading computed
 *    values, so only stylesheet-owned values survive that path.
 */

import type { NotificationCategory } from "@/services/notificationService";

const STYLE_ID = "notif-category-tokens";

/** Hue offsets (fraction of the wheel) from the accent hue. None is 0 — the
 *  accent is reserved for global affordances and may not be a category colour. */
const HUE_OFFSETS: Record<NotificationCategory, number> = {
  session_compaction: 0.1,
  memory_consolidation: 0.22,
  pipeline: 0.34,
  dictation: 0.45,
  hardware: 0.56,
  models: 0.7,
  storage: 0.86,
};

export const NOTIFICATION_CATEGORIES = Object.keys(
  HUE_OFFSETS
) as NotificationCategory[];

/* ── Colour math (no three.js dependency — pure HSL) ──────────────────── */

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
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

function hue2rgb(p: number, q: number, t: number): number {
  let x = t;
  if (x < 0) x += 1;
  if (x > 1) x -= 1;
  if (x < 1 / 6) return p + (q - p) * 6 * x;
  if (x < 1 / 2) return q;
  if (x < 2 / 3) return p + (q - p) * (2 / 3 - x) * 6;
  return p;
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  if (s === 0) {
    const v = Math.round(l * 255);
    return [v, v, v];
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  return [
    Math.round(hue2rgb(p, q, h + 1 / 3) * 255),
    Math.round(hue2rgb(p, q, h) * 255),
    Math.round(hue2rgb(p, q, h - 1 / 3) * 255),
  ];
}

/* ── Derivation ───────────────────────────────────────────────────────── */

/**
 * Derives the seven category triplets from an accent triplet (`"r, g, b"`).
 * Near-monochrome accents fall back to a pleasant base hue and desaturate
 * toward the accent instead of exploding to full saturation (same guard the
 * legend palette applies).
 */
export function deriveNotificationCategoryTokens(
  accentTriplet: string,
  isLight: boolean
): Record<NotificationCategory, string> {
  const parts = accentTriplet.split(",").map((s) => parseInt(s.trim(), 10));
  const valid = parts.length === 3 && !parts.some((n) => isNaN(n));
  const r = valid ? parts[0] : 0;
  const g = valid ? parts[1] : 219;
  const b = valid ? parts[2] : 233;

  const [rawH, rawS] = rgbToHsl(r / 255, g / 255, b / 255);

  const GRAY_ACCENT_SAT_SCALE = 0.3;
  const satScale = rawS < 0.1 ? GRAY_ACCENT_SAT_SCALE : 1;
  const baseH = rawS < 0.1 ? 0.55 : rawH;

  const s = (isLight ? 0.88 : 0.94) * satScale;
  const l = isLight ? 0.44 : 0.64;

  const out = {} as Record<NotificationCategory, string>;
  for (const category of NOTIFICATION_CATEGORIES) {
    const h = ((baseH + HUE_OFFSETS[category]) % 1 + 1) % 1;
    const [cr, cg, cb] = hslToRgb(h, s, l);
    out[category] = `${cr}, ${cg}, ${cb}`;
  }
  return out;
}

/**
 * Writes the derived family into a dedicated stylesheet on the document.
 * Called from the theme module on every theme/accent write.
 */
export function applyNotificationCategoryTokens(
  accentTriplet: string,
  isLight: boolean
): void {
  const tokens = deriveNotificationCategoryTokens(accentTriplet, isLight);
  // CSS custom properties are hyphenated (`--notif-session-compaction`):
  // Tailwind arbitrary values turn `_` into a space, so an underscored name
  // would break every `var()` reference in a class string.
  const body = NOTIFICATION_CATEGORIES.map(
    (c) => `  --notif-${c.replace(/_/g, "-")}: ${tokens[c]};`
  ).join("\n");
  const css = `:root {\n${body}\n}\n`;

  let style = document.getElementById(STYLE_ID) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement("style");
    style.id = STYLE_ID;
    document.head.appendChild(style);
  }
  if (style.textContent !== css) style.textContent = css;
}
