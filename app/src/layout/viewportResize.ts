/**
 * Viewport-resize gate. Mirrors the theme-flip gate (`data-theme-transition`)
 * for window resizes: while a resize is in flight, `data-viewport-resize` on
 * `<html>` suspends CSS transitions, pauses keyframe animations, and drops
 * `backdrop-filter` app-wide (see `index.css`), so minimize / maximize /
 * drag-resize reflows as a cheap static layout instead of a 60 fps
 * compositing storm. Unlike the theme gate this is a *performance* gate, not
 * a motion preference — it applies regardless of `prefers-reduced-motion`.
 *
 * Listener discipline follows `.agents/rules/frontend-style-guide.md` §4.4:
 * one rAF-coalesced `resize` listener, tracked settle timer, idempotent
 * install. Consumers that need per-frame width updates subscribe instead of
 * adding their own `resize` listeners.
 */
import { BREAKPOINT_COMPACT_MAX } from "./breakpoints";

export type ViewportLayout = "compact" | "wide";

/** Quiet period after the last resize event before the gate reopens. */
export const RESIZE_SETTLE_MS = 180;

const RESIZE_ATTR = "data-viewport-resize";

let installed = false;
let rafId: number | null = null;
let settleTimer: ReturnType<typeof setTimeout> | null = null;
let resizing = false;

/** Fired on every rAF-coalesced resize frame (width/height may have changed). */
const frameListeners = new Set<() => void>();
/** Fired only on gate transitions (true = resize started, false = settled). */
const gateListeners = new Set<(active: boolean) => void>();

function setResizing(next: boolean): void {
  if (resizing === next) return;
  resizing = next;
  if (typeof document !== "undefined") {
    if (next) document.documentElement.setAttribute(RESIZE_ATTR, "");
    else document.documentElement.removeAttribute(RESIZE_ATTR);
  }
  gateListeners.forEach((listener) => listener(next));
}

function onFrame(): void {
  rafId = null;
  frameListeners.forEach((listener) => listener());
  // Re-arm the settle window on every frame: the gate closes only after a
  // full RESIZE_SETTLE_MS of quiet, so a continuous drag holds it open.
  if (settleTimer !== null) clearTimeout(settleTimer);
  settleTimer = setTimeout(() => {
    settleTimer = null;
    setResizing(false);
  }, RESIZE_SETTLE_MS);
}

export function installViewportResizeGate(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  window.addEventListener(
    "resize",
    () => {
      setResizing(true);
      if (rafId !== null) return;
      rafId = requestAnimationFrame(onFrame);
    },
    { passive: true }
  );
}

/** True while a resize is in flight (gate attribute present). */
export function isViewportResizing(): boolean {
  return resizing;
}

export function getViewportLayout(
  width: number = typeof window !== "undefined" ? window.innerWidth : BREAKPOINT_COMPACT_MAX
): ViewportLayout {
  return width < BREAKPOINT_COMPACT_MAX ? "compact" : "wide";
}

export function subscribeViewportFrame(listener: () => void): () => void {
  frameListeners.add(listener);
  return () => {
    frameListeners.delete(listener);
  };
}

export function subscribeViewportGate(listener: (active: boolean) => void): () => void {
  gateListeners.add(listener);
  return () => {
    gateListeners.delete(listener);
  };
}
