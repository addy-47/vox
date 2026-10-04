/**
 * Single source of truth for layout-mode breakpoints.
 *
 * `BREAKPOINT_COMPACT_MAX` is the compact/wide boundary: below 1024px the
 * monitoring button moves from the bottom-left corner into the EdgeNav, the
 * floor feather goes full-width, and two-column sheets collapse. It is the
 * same boundary as Tailwind's `lg` (declared explicitly in
 * `tailwind.config.js`, and pinned to it by Invariant 26).
 *
 * Never compare against a bare `1024` — use `isCompactWidth()`.
 */
export const BREAKPOINT_COMPACT_MAX = 1024;

/** Below this width, opposite edge rails can no longer coexist (gap < 40px). */
export const BREAKPOINT_OPPOSITE_COLLISION_MAX = 480;

/** Below this height, compact-height treatments apply (unused — reserved). */
export const BREAKPOINT_COMPACT_HEIGHT_MAX = 640;

/** Below this width, the dual edge-rail exclusivity rule kicks in. */
export const BREAKPOINT_DUAL_PANEL_MIN = 1280;

/** Below this width, mobile-screen treatments apply (Home orb scaling). */
export const BREAKPOINT_MOBILE_MAX = 768;

function currentWidth(fallback: number): number {
  return typeof window !== "undefined" ? window.innerWidth : fallback;
}

export function isCompactWidth(width?: number): boolean {
  return (width ?? currentWidth(BREAKPOINT_COMPACT_MAX)) < BREAKPOINT_COMPACT_MAX;
}

export function isMobileWidth(width?: number): boolean {
  return (width ?? currentWidth(BREAKPOINT_MOBILE_MAX)) < BREAKPOINT_MOBILE_MAX;
}
