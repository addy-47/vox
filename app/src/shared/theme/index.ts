/**
 * Theme application — the single owner of every `<html>` write that a
 * dark<->light flip performs.
 *
 * The flip is a pure CSS color transition, gated by `data-theme-transition` on
 * `<html>` (see "Theme Transition Mechanism" in `index.css`).
 *
 * ## Why there is no View Transitions API here
 *
 * `document.startViewTransition()` was implemented and measured on this app's
 * own target engine (WebKitGTK 2.52). It was ~1 second of frozen UI before the
 * theme changed at all, because the API rasterizes the entire viewport *before*
 * invoking the update callback, and this page is expensive to rasterize: ~2000
 * nodes, a 60vmax radial-gradient ambient field, `mix-blend-mode: multiply` on a
 * fixed z-9999 overlay, a WebGL orb, 5 infinite full-viewport animations, and
 * dozens of `backdrop-filter` regions. The snapshot dominates and the callback
 * runs after it, so the perceived latency was worse than the un-animated bug it
 * replaced.
 *
 * A gated color transition has no capture step: the flip is a synchronous DOM
 * write and the browser interpolates the paint-only properties directly. The
 * gate costs one style recalc when it opens and one when it closes.
 *
 * Spec: docs/plans/phase12/theme-transition-rca-and-remediation.md
 */

import { hexToRgb } from "@/shared/lib/utils";
import { applyNotificationCategoryTokens } from "./notificationCategoryTokens";
import { beginFlipTrace, endFlipTrace, traceFlipTargets } from "./trace";

/**
 * Custom-property flip: the element-level `transition-property` fan-out was
 * removed (it forced WebKitGTK to build a transition per element per changed
 * color — a ~230ms recalc on heavy routes). Instead the ~33 theme tokens on
 * `<html>` are interpolated once per frame at :root; every surface that reads
 * `var(--token)` re-resolves and repaints, giving the same perceptual fade
 * with none of the per-element transition machinery.
 */
const FLIP_TOKEN_KEYS = [
  "background", "foreground", "foreground-muted", "accent", "accent-dark",
  "accent-muted", "accent-foreground", "card", "border", "sidebar", "field",
  "signal", "field-energy", "connection-glow", "connection-core",
  "hub-connector-active-opacity", "hub-connector-tick35-opacity",
  "hub-connector-tick65-opacity",
  "success", "success-dark", "error", "error-dark", "danger", "danger-dark",
  "warning", "warning-dark", "warn-soft", "info", "info-dark", "violet",
  "violet-dark", "pink", "muted", "muted-soft",
  "shadow-color", "shadow-alpha", "highlight-color", "highlight-alpha",
  // Accent-derived notification category family (notifications-spec §4.7)
  "notif-session-compaction", "notif-memory-consolidation", "notif-pipeline",
  "notif-dictation", "notif-hardware", "notif-models", "notif-storage",
];

type TokenValue =
  | { kind: "triplet"; r: number; g: number; b: number }
  | { kind: "alpha"; alpha: number }
  | { kind: "number"; v: number };

function parseTokenLiteral(raw: string): TokenValue | null {
  const t = raw.trim();
  if (/^[\d.]+$/.test(t)) return { kind: "number", v: parseFloat(t) };
  const triplet = t.match(/^(\d+)\s*,\s*(\d+)\s*,\s*(\d+)$/);
  if (triplet) {
    return { kind: "triplet", r: +triplet[1], g: +triplet[2], b: +triplet[3] };
  }
  const alpha = t.match(/rgba\(var\(--accent\),\s*([\d.]+)\)/);
  if (alpha) return { kind: "alpha", alpha: parseFloat(alpha[1]) };
  return null;
}

function lerpToken(
  from: TokenValue,
  to: TokenValue,
  e: number,
  accentTriple: string
): string {
  if (from.kind !== to.kind) return "";
  if (from.kind === "triplet" && to.kind === "triplet") {
    return `${Math.round(from.r + (to.r - from.r) * e)}, ${Math.round(
      from.g + (to.g - from.g) * e
    )}, ${Math.round(from.b + (to.b - from.b) * e)}`;
  }
  if (from.kind === "number" && to.kind === "number") {
    return String(from.v + (to.v - from.v) * e);
  }
  if (from.kind === "alpha" && to.kind === "alpha") {
    return `rgba(${accentTriple}, ${from.alpha + (to.alpha - from.alpha) * e})`;
  }
  return "";
}

/** cubic-bezier(0.4, 0, 0.2, 1) — same curve as --theme-ease. */
function cubicBezierEase(t: number): number {
  const cx = 3 * 0.4;
  const bx = 3 * (0.2 - 0.4) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * 0;
  const by = 3 * (1 - 0) - cy;
  const ay = 1 - cy - by;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 24; i++) {
    const mx = (lo + hi) / 2;
    const x = ((ax * mx + bx) * mx + cx) * mx;
    if (x < t) lo = mx;
    else hi = mx;
  }
  const tt = (lo + hi) / 2;
  return ((ay * tt + by) * tt + cy) * tt;
}

let flipGeneration = 0;

/**
 * Target cache: the post-write literal map for a theme+accent is stylesheet-
 * determined and never changes at runtime, so only the first flip into a
 * theme pays the second forced recalc. Keyed on accent_seed too, since
 * --accent rides inline. Bounded; accents change rarely.
 */
const targetCache = new Map<string, Map<string, string>>();
const TARGET_CACHE_LIMIT = 12;

function targetCacheKey(appearance: ThemeAppearance): string {
  return `${appearance.theme}::${appearance.accent_seed}`;
}

/**
 * Interpolates FLIP_TOKEN_KEYS on <html> from the current (old-theme) values to
 * the new-theme values, over THEME_TRANSITION_MS frames. Start values are
 * captured before writeThemeToDom flips the attributes; the flip's target
 * values are read from the freshly-written attributes, then instantly masked
 * by inline overrides at their start values, so nothing paints mid-write.
 */
function readRootTokenValues(): Map<string, string> {
  const styles = getComputedStyle(document.documentElement);
  const map = new Map<string, string>();
  for (const key of FLIP_TOKEN_KEYS) {
    map.set(key, styles.getPropertyValue(`--${key}`).trim());
  }
  return map;
}

function animateTokensToNewTheme(
  startValues: Map<string, string>,
  endValues: Map<string, string>,
  newAccentInline: string,
  durationMs: number
) {
  const gen = ++flipGeneration;
  const root = document.documentElement;

  const parsed = FLIP_TOKEN_KEYS.map((key) => ({
    key,
    from: parseTokenLiteral(startValues.get(key) ?? ""),
    to: parseTokenLiteral(endValues.get(key) ?? ""),
  })).filter((p) => p.from && p.to);

  const t0 = performance.now();
  // 30fps write pacing: restyling 37 tokens on :root forces a full-document
  // recalc per write, so writes land on alternate frames. The clock keeps
  // running at rAF rate and the final frame always writes exact end values.
  let frame = 0;
  const tick = (now: number) => {
    if (gen !== flipGeneration) return;
    const t = Math.min(1, (now - t0) / durationMs);
    frame += 1;
    if (t < 1 && frame % 2 === 0) {
      requestAnimationFrame(tick);
      return;
    }
    const e = cubicBezierEase(t);

    const accent = parsed.find((p) => p.key === "accent");
    const accentTriple =
      accent && accent.from && accent.to
        ? lerpToken(accent.from, accent.to, e, "")
        : "";

    for (const p of parsed) {
      const out = lerpToken(p.from!, p.to!, e, accentTriple);
      if (out) root.style.setProperty(`--${p.key}`, out);
    }

    if (t < 1) {
      requestAnimationFrame(tick);
    } else {
      for (const key of FLIP_TOKEN_KEYS) root.style.removeProperty(`--${key}`);
      root.style.setProperty("--accent", newAccentInline);
    }
  };
  requestAnimationFrame(tick);
}

export interface ThemeAppearance {
  theme: string;
  accent_seed: string;
}

/**
 * Must stay in sync with `--theme-transition-ms` in `index.css`. Invariant 11 in
 * `test/invariants.test.ts` asserts the two agree.
 */
export const THEME_TRANSITION_MS = 200;

/** One frame of slack so the final interpolated frame renders before the gate
 * reopens and per-component durations resume. */
const TRANSITION_TAIL_MS = 32;

/** Rapid toggling must not extend the flip window.
 *
 * The window is 232ms of suspended `backdrop-filter` and a frozen ambient field
 * (see "Theme Transition Mechanism" in `index.css`). Re-arming the close timer on
 * every click made that window `232ms after the LAST click` rather than after
 * the flip, so a burst of clicks held every glass surface flat for as long as
 * the user kept clicking — the flat-then-pop glitch scaled with click rate.
 *
 * Past `FLIP_BURST_LIMIT` flips inside `FLIP_BURST_WINDOW_MS` the in-flight
 * window keeps its original close time. The final flip of a burst may then be
 * cut off mid-fade: the theme still lands, it just may not get the full 200ms.
 * That is the cheaper trade than a half-second of flat glass. */
const FLIP_BURST_LIMIT = 3;
const FLIP_BURST_WINDOW_MS = 500;

let lastAppliedTheme: string | null = null;
let transitionTimer: ReturnType<typeof setTimeout> | null = null;
let burstFlipCount = 0;
let burstWindowStart = 0;

const transitionListeners = new Set<() => void>();
let isTransitioning = false;

/** True while a flip is interpolating. Consumers that must not animate their own
 * layers during the flip (the ambient field's rAF loop) park themselves here. */
export function getThemeTransitioning(): boolean {
  return isTransitioning;
}

export function subscribeThemeTransition(listener: () => void): () => void {
  transitionListeners.add(listener);
  return () => {
    transitionListeners.delete(listener);
  };
}

function setTransitioning(next: boolean) {
  if (isTransitioning === next) return;
  isTransitioning = next;
  transitionListeners.forEach((listener) => listener());
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function closeGate() {
  if (transitionTimer) {
    clearTimeout(transitionTimer);
    transitionTimer = null;
  }
  if (typeof document !== "undefined") {
    document.documentElement.removeAttribute("data-theme-transition");
  }
}

/** Synchronous by contract — the flip must land in the same task as the click. */
function writeThemeToDom(appearance: ThemeAppearance) {
  const root = document.documentElement;
  root.setAttribute("data-theme", appearance.theme);
  root.style.setProperty("--accent", hexToRgb(appearance.accent_seed));
  applyNotificationCategoryTokens(
    hexToRgb(appearance.accent_seed),
    appearance.theme === "light"
  );
  if (appearance.theme === "light") {
    root.classList.add("light");
    root.classList.remove("dark");
  } else {
    root.classList.add("dark");
    root.classList.remove("light");
  }
}

function persistForBootScript(appearance: ThemeAppearance) {
  // The pre-React inline script in index.html replays theme + accent_seed
  // before first paint. Without this the boot loader renders the default cyan
  // and the UI flashes on launch.
  try {
    localStorage.setItem(
      "vox_appearance",
      JSON.stringify({ theme: appearance.theme, accent_seed: appearance.accent_seed })
    );
  } catch {
    /* localStorage unavailable; first-paint flash will be a no-op */
  }
}

export interface ApplyThemeOptions {
  /** Animate the flip. Defaults to true. Boot and discard paths that must not
   * read as a user-initiated transition pass `false`. */
  animate?: boolean;
}

/**
 * Applies `appearance` to the document.
 *
 * The first call after module init is never treated as a flip — there is no
 * previous theme to interpolate from, so `lastAppliedTheme` starts null and the
 * caller gets an instant write. Every later theme *change* animates; a repeat of
 * the same theme (e.g. an accent-only autosave round-trip) writes without
 * animating so it can never re-fire a stale fade.
 */
export function applyTheme(
  appearance: ThemeAppearance | undefined,
  options: ApplyThemeOptions = {}
): void {
  if (!appearance || typeof document === "undefined") return;

  const { animate = true } = options;
  const trace = beginFlipTrace();
  const isThemeChange = lastAppliedTheme !== null && lastAppliedTheme !== appearance.theme;
  lastAppliedTheme = appearance.theme;

  const root = document.documentElement;

  if (!animate || !isThemeChange || prefersReducedMotion()) {
    closeGate();
    setTransitioning(false);
    writeThemeToDom(appearance);
    persistForBootScript(appearance);
    endFlipTrace(trace, false);
    return;
  }

  // Open the gate and flip underneath it in the same task, so the interpolated
  // first frame is the very next paint. Re-arming an already-open gate (rapid
  // toggling) only resets the timer — removing and re-adding the attribute would
  // cost two extra full-document style recalcs per toggle.
  const now = typeof performance !== "undefined" ? performance.now() : Date.now();
  if (now - burstWindowStart > FLIP_BURST_WINDOW_MS) {
    burstWindowStart = now;
    burstFlipCount = 0;
  }
  burstFlipCount += 1;

  if (transitionTimer) {
    // Bursted: let the pending close stand so the window cannot grow with
    // click rate. The flip below still lands, it may just fade shorter.
    if (burstFlipCount <= FLIP_BURST_LIMIT) {
      clearTimeout(transitionTimer);
      transitionTimer = null;
    }
  } else {
    root.setAttribute("data-theme-transition", "true");
  }

  setTransitioning(true);
  const startValues = readRootTokenValues();
  writeThemeToDom(appearance);

  // Drop any masks left by a burst flip so the new theme's literal values
  // are readable, then snapshot the true targets — unless the cache already
  // holds them, in which case the second forced recalc is skipped entirely.
  // --accent stays inline (writeThemeToDom owns it) so its target reads the
  // user's accent_seed.
  const cacheKey = targetCacheKey(appearance);
  let endValues = targetCache.get(cacheKey);
  if (!endValues) {
    for (const key of FLIP_TOKEN_KEYS) {
      if (key !== "accent") root.style.removeProperty(`--${key}`);
    }
    endValues = readRootTokenValues();
    if (targetCache.size >= TARGET_CACHE_LIMIT) targetCache.clear();
    targetCache.set(cacheKey, endValues);
  }
  const newAccentInline = root.style.getPropertyValue("--accent");

  // Mask the flip: re-apply the old values inline so nothing new paints until
  // the first interpolated frame. The rAF loop's first tick overwrites them.
  for (const [key, value] of startValues) {
    root.style.setProperty(`--${key}`, value);
  }

  animateTokensToNewTheme(startValues, endValues, newAccentInline, THEME_TRANSITION_MS);
  persistForBootScript(appearance);
  const baseTargets = [
    "body",
    "[data-edge-nav]",
    '[data-flip-trace="monitor-btn"]',
    '[data-flip-trace="title-bar-dot"]',
    '[data-spatial-zone="cluster"] button',
  ];
  const cardIds = ["persona", "models", "working_memory", "personal_memory", "appearance", "interaction"];
  const cardTargets = cardIds
    .filter((id) => document.getElementById(`card-${id}`))
    .map((id) => `#card-${id} .glass-card, #card-${id} .glass`);
  traceFlipTargets(trace.id, [...baseTargets, ...cardTargets]);
  endFlipTrace(trace, animate);

  if (!transitionTimer) {
    transitionTimer = setTimeout(() => {
      transitionTimer = null;
      root.removeAttribute("data-theme-transition");
      setTransitioning(false);
    }, THEME_TRANSITION_MS + TRANSITION_TAIL_MS);
  }
}

/** Test seam: forgets the last applied theme so the next call is a first paint. */
export function resetThemeStateForTests() {
  lastAppliedTheme = null;
  burstFlipCount = 0;
  burstWindowStart = 0;
  closeGate();
  setTransitioning(false);
}