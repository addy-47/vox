/**
 * Theme application — the single owner of every `<html>` write that a
 * dark<->light flip or an accent change performs.
 *
 * The flip is a pure CSS color transition, gated by `data-theme-transition` on
 * `<html>` (see "Theme Transition Mechanism" in `index.css`). The same gate is
 * held for the duration of an accent-preview drag, because that writes the same
 * `:root` custom properties and would otherwise pay the same costs — 237
 * `transition-all` sites, the ambient field's animations, and every
 * `backdrop-filter` region — on every mouse move.
 *
 * ## Why there is no View Transitions API here
 *
 * `document.startViewTransition()` was implemented and measured on this app's
 * own target engine (WebKitGTK 2.52). It was ~1 second of frozen UI before the
 * theme changed at all, because the API rasterizes the entire viewport *before*
 * invoking the update callback, and this page is expensive to rasterize: ~9,600
 * nodes with every Settings card open, a 60vmax radial-gradient ambient field,
 * a WebGL orb on some routes, and dozens of `backdrop-filter` regions. The
 * snapshot dominates and the callback runs after it, so the perceived latency
 * was worse than the un-animated bug it replaced.
 *
 * A gated color transition has no capture step: the flip is a synchronous DOM
 * write and the browser interpolates the paint-only properties directly. The
 * gate costs one style recalc when it opens and one when it closes.
 *
 * ## Why this module exists
 *
 * Writing a `:root` custom property invalidates the computed value of every
 * descendant that references it. Measured on the Settings route with all six
 * cards open (9,606 nodes, ~1,500 `var(--accent)` references): one appearance
 * write costs 683-2772ms of full-document style recalc. Against a 315-node
 * document the same write costs 0.0ms. Node count and reference count are the
 * multipliers, so every appearance write is funnelled through here — once per
 * frame, inside the repaint gate, with the accent-derived token family kept in
 * sync — rather than being issued ad hoc from whichever component noticed the
 * change first.
 *
 * Spec: docs/plans/phase12/theme-transition-rca-and-remediation.md
 */

import { hexToRgb } from "@/shared/lib/utils";
import {
  applyNotificationCategoryTokens,
  NOTIFICATION_TOKEN_PROPS,
} from "./notificationCategoryTokens";
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
  // Ambient background gradient stops (interpolated continuously to prevent backdrop snap)
  "amb-stop-0", "amb-stop-1", "amb-stop-2",
  // Glass tokens used by session cards and panels
  "glass-surface", "glass-deep", "ghost", "glass-tint",
  // History central clock hub and pill stops
  "clock-hub-stop-0", "clock-hub-stop-1", "clock-hub-stop-2", "clock-pill-bg",
  // Clock and orbit card shadow & border alphas
  "clock-hub-shadow-alpha", "clock-hub-glow-alpha", "clock-hub-inset-alpha",
  "orbit-card-border-alpha", "orbit-card-shadow-alpha", "orbit-card-inset-alpha",
  "orbit-card-selected-border-alpha", "orbit-card-selected-glow-alpha",
];

type TokenValue =
  | { kind: "triplet"; r: number; g: number; b: number }
  | { kind: "rgba"; r: number; g: number; b: number; a: number }
  | { kind: "alpha"; alpha: number }
  | { kind: "number"; v: number };

function parseTokenLiteral(raw: string): TokenValue | null {
  const t = raw.trim();
  if (/^[\d.]+$/.test(t)) return { kind: "number", v: parseFloat(t) };
  const triplet = t.match(/^(\d+)\s*,\s*(\d+)\s*,\s*(\d+)$/);
  if (triplet) {
    return { kind: "triplet", r: +triplet[1], g: +triplet[2], b: +triplet[3] };
  }
  // `getComputedStyle` resolves `var()` before returning, so a token declared
  // as `rgba(var(--accent), 0.15)` reads back as `rgba(0, 219, 233, 0.15)` —
  // a four-channel form. Matching only the bare triplet silently dropped
  // every such token from the interpolation, which is why `--connection-glow`
  // and `--connection-core` snapped at t=0 while their neighbours faded.
  const rgba = t.match(
    /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$/
  );
  if (rgba) {
    return {
      kind: "rgba",
      r: +rgba[1],
      g: +rgba[2],
      b: +rgba[3],
      a: rgba[4] === undefined ? 1 : +rgba[4],
    };
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
  if (from.kind === "triplet" && to.kind === "triplet") {
    return `${Math.round(from.r + (to.r - from.r) * e)}, ${Math.round(
      from.g + (to.g - from.g) * e
    )}, ${Math.round(from.b + (to.b - from.b) * e)}`;
  }
  if (from.kind === "rgba" && to.kind === "rgba") {
    return `rgba(${Math.round(from.r + (to.r - from.r) * e)}, ${Math.round(
      from.g + (to.g - from.g) * e
    )}, ${Math.round(from.b + (to.b - from.b) * e)}, ${(
      from.a +
      (to.a - from.a) * e
    ).toFixed(3)})`;
  }
  if (from.kind === "number" && to.kind === "number") {
    return String(from.v + (to.v - from.v) * e);
  }
  if (from.kind === "alpha" && to.kind === "alpha") {
    return `rgba(${accentTriple}, ${(from.alpha + (to.alpha - from.alpha) * e).toFixed(3)})`;
  }
  if (
    (from.kind === "alpha" || from.kind === "rgba") &&
    (to.kind === "alpha" || to.kind === "rgba")
  ) {
    const fromAlpha = from.kind === "alpha" ? from.alpha : from.a;
    const toAlpha = to.kind === "alpha" ? to.alpha : to.a;
    return `rgba(${accentTriple}, ${(fromAlpha + (toAlpha - fromAlpha) * e).toFixed(3)})`;
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
const TARGET_CACHE_LIMIT = 32;

/**
 * Tokens this module owns as INLINE custom properties on `<html>`, and which
 * must therefore survive the end-value capture's inline strip.
 *
 * `--accent` was already inline. The `--notif-*` family was migrated from a
 * `<style>` rule to inline props (see notificationCategoryTokens.ts) so that
 * writing an appearance change no longer invalidates every CSS rule in the
 * document. Stripping them here would erase the very values the capture is
 * trying to read.
 */
const INLINE_OWNED_TOKENS: ReadonlySet<string> = new Set([
  "accent",
  ...NOTIFICATION_TOKEN_PROPS.map((prop) => prop.replace(/^--/, "")),
]);

function rememberTargets(
  cacheKey: string,
  endValues: Map<string, string>
): void {
  // Evict oldest, not the whole map. A full `clear()` on every miss meant a
  // single accent drag could discard every previously-learned theme target,
  // so the next flip paid its second forced recalc for no reason.
  while (targetCache.size >= TARGET_CACHE_LIMIT) {
    const oldest = targetCache.keys().next();
    if (oldest.done) break;
    targetCache.delete(oldest.value);
  }
  targetCache.set(cacheKey, endValues);
}

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
  // 30fps write pacing: restyling the token set on :root forces a full-document
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

/**
 * The `data-theme-transition` attribute has two independent holders: the flip
 * itself, and an in-progress accent preview. Either one keeps the gate open.
 * They were separate code paths before; unifying them means an accent drag
 * that ends in a theme commit never churns the attribute, because the preview
 * is still holding it when the commit's `applyTheme` runs.
 */
let flipGateHeld = false;
let previewGateHeld = false;
let isGateOpen = false;

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

/**
 * Opens or closes the repaint gate to match its two holders.
 *
 * No-op when the gate is already in the requested state: re-writing the
 * attribute while open is idempotent but pointless, and closing then reopening
 * costs two extra full-document style recalcs.
 */
function syncGate() {
  const shouldBeOpen = flipGateHeld || previewGateHeld;
  if (shouldBeOpen === isGateOpen) return;
  isGateOpen = shouldBeOpen;
  if (typeof document !== "undefined") {
    const root = document.documentElement;
    if (shouldBeOpen) {
      root.setAttribute("data-theme-transition", "true");
    } else {
      root.removeAttribute("data-theme-transition");
    }
  }
  setTransitioning(shouldBeOpen);
}

function closeGate() {
  if (transitionTimer) {
    clearTimeout(transitionTimer);
    transitionTimer = null;
  }
  flipGateHeld = false;
  syncGate();
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
  }
  flipGateHeld = true;
  syncGate();

  const startValues = readRootTokenValues();
  const ambEl = document.querySelector(".amb-base"); // TEMP-DIAGNOSTIC: pre-flip backdrop baseline. Revert with probe default.
  const preAmbImage =
    ambEl && import.meta.env.DEV ? getComputedStyle(ambEl).backgroundImage.slice(0, 120) : "";
  writeThemeToDom(appearance);

  // Drop any masks left by a burst flip so the new theme's literal values
  // are readable, then snapshot the true targets — unless the cache already
  // holds them, in which case the second forced recalc is skipped entirely.
  // Inline-owned tokens (--accent and the --notif-* family) survive the strip,
  // because stripping them would erase the values this capture is reading.
  const cacheKey = targetCacheKey(appearance);
  let endValues = targetCache.get(cacheKey);
  if (!endValues) {
    for (const key of FLIP_TOKEN_KEYS) {
      if (INLINE_OWNED_TOKENS.has(key)) continue;
      root.style.removeProperty(`--${key}`);
    }
    endValues = readRootTokenValues();
    rememberTargets(cacheKey, endValues);
  }
  const newAccentInline = root.style.getPropertyValue("--accent");

  // Mask the flip: re-apply the old values inline so nothing new paints until
  // the first interpolated frame. The rAF loop's first tick overwrites them.
  for (const [key, value] of startValues) {
    root.style.setProperty(`--${key}`, value);
  }

  if (ambEl && import.meta.env.DEV) {
    const postAmbImage = getComputedStyle(ambEl).backgroundImage.slice(0, 120);
    console.info(
      `[theme-flip] #${trace.id} amb-base same-task snap pre!=post: ${preAmbImage !== postAmbImage} pre=${preAmbImage} post=${postAmbImage}`
    );
  }

  animateTokensToNewTheme(startValues, endValues, newAccentInline, THEME_TRANSITION_MS);
  persistForBootScript(appearance);
  const baseTargets = [
    "body",
    ".amb-base",
    ".amb-glow",
    "[data-edge-nav]",
    '[data-flip-trace="monitor-btn"]',
    '[data-flip-trace="title-bar-dot"]',
    '[data-spatial-zone="cluster"] button',
    ".clock-hub",
    ".clock-pill",
    ".orbit-card-surface",
  ];
  const cardIds = ["persona", "models", "working_memory", "personal_memory", "appearance", "interaction"];
  const cardTargets = cardIds
    .filter((id) => document.getElementById(`card-${id}`))
    .map((id) => `#card-${id} .glass-card, #card-${id} .glass`);
  traceFlipTargets(trace.id, [...baseTargets, ...cardTargets], THEME_TRANSITION_MS + TRANSITION_TAIL_MS);
  endFlipTrace(trace, animate);

  if (!transitionTimer) {
    transitionTimer = setTimeout(() => {
      transitionTimer = null;
      flipGateHeld = false;
      syncGate();
    }, THEME_TRANSITION_MS + TRANSITION_TAIL_MS);
  }
}

/* ── Accent preview ───────────────────────────────────────────────────────
 *
 * The live accent drag is the single most expensive interaction in the app.
 * It writes `--accent` on `<html>`, which every descendant inherits: measured
 * on the Settings route, that one write forces a 683-2772ms document style
 * recalc (9,606 nodes, ~1,500 `var(--accent)` references, no CSS containment
 * anywhere in the app).
 *
 * Three things make it survivable, and all three live here so this stays the
 * single owner of every `<html>` write a colour change performs:
 *
 *  1. Writes are rAF-coalesced. react-colorful fires `onChange` from an
 *     unthrottled `document` mousemove listener, which routinely emits several
 *     events per frame. Coalescing caps the invalidation at one per frame.
 *  2. The repaint gate is held open for the whole drag. The theme flip already
 *     knows how to suspend `transition-all` on 237 sites, pause the ambient
 *     field's animations, and drop every `backdrop-filter` region; the drag had
 *     no such protection and so paid all three costs on every frame.
 *  3. The accent-derived `--notif-*` family is written inline alongside
 *     `--accent`, so the seven notification colours track the drag live instead
 *     of jumping on release.
 */

let previewAccentFrame: number | null = null;
let pendingAccentSeed: string | null = null;

/** Opens the repaint gate for the duration of an accent drag. */
export function beginAccentPreview(): void {
  previewGateHeld = true;
  syncGate();
}

/**
 * Queues a live accent write. Coalesced to at most one write per animation
 * frame; the newest value wins. Call inside a drag's change handler and pair
 * with `endAccentPreview()` on release.
 */
export function previewAccent(accentSeed: string): void {
  if (typeof document === "undefined") return;
  pendingAccentSeed = accentSeed;
  if (previewAccentFrame !== null) return;

  previewAccentFrame = requestAnimationFrame(() => {
    previewAccentFrame = null;
    const seed = pendingAccentSeed;
    if (seed === null || typeof document === "undefined") return;

    const root = document.documentElement;
    const triplet = hexToRgb(seed);
    // `--accent` and the notification family are set back to back, before the
    // browser gets a chance to recalc, so this is one style recalc per frame
    // rather than eight.
    root.style.setProperty("--accent", triplet);
    applyNotificationCategoryTokens(
      triplet,
      root.getAttribute("data-theme") === "light"
    );
  });
}

/** Releases the gate and drops any queued write. Safe to call repeatedly. */
export function endAccentPreview(): void {
  if (previewAccentFrame !== null) {
    cancelAnimationFrame(previewAccentFrame);
    previewAccentFrame = null;
  }
  pendingAccentSeed = null;
  previewGateHeld = false;
  syncGate();
}

/** Test seam: forgets the last applied theme so the next call is a first paint. */
export function resetThemeStateForTests() {
  lastAppliedTheme = null;
  burstFlipCount = 0;
  burstWindowStart = 0;
  previewGateHeld = false;
  pendingAccentSeed = null;
  if (previewAccentFrame !== null) {
    cancelAnimationFrame(previewAccentFrame);
    previewAccentFrame = null;
  }
  closeGate();
  setTransitioning(false);
}
