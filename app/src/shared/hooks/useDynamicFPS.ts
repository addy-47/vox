import { useRef, useEffect, useState, useCallback } from 'react';

interface DynamicFPSOptions {
  /** Callback receives deltaTime in ms since last non-skipped frame */
  onFrame: (deltaTime: number) => void;
  /** Whether the component is visible (caller-provided boolean) */
  isVisible?: boolean;
  /** Whether the page is visible (document.visibilityState, caller-provided) */
  isPageVisible?: boolean;
  /**
   * Element to observe for visibility. When provided, the hook builds its own
   * IntersectionObserver and ANDs it with `isVisible` — previously the hook's
   * contract claimed this and no observer was ever constructed.
   */
  observeRef?: React.RefObject<Element | null>;
  /**
   * When true, the hook listens to `document.visibilitychange` itself and ANDs
   * it with `isPageVisible`. Opt-in so existing consumers keep their behaviour.
   */
  trackPageVisibility?: boolean;
  /** FPS target when fully active (default: 60) */
  fpsActive?: number;
  /** FPS target when idle (default: 15) */
  fpsIdle?: number;
  /** Whether the component is in "active" state vs "idle" */
  isActive?: boolean;
  /** Whether to fully pause (0fps, e.g. sleeping) */
  isPaused?: boolean;
}

/**
 * Manages a requestAnimationFrame loop with dynamic frame-rate targeting.
 *
 * Frame-skipping algorithm:
 *   frameInterval = 1000 / targetFps
 *   On each RAF: if elapsed < frameInterval → skip call to onFrame
 *   else → call onFrame(delta), reset timer
 *
 * Edge cases:
 *   - isPaused=true OR isPageVisible=false → cancels RAF entirely
 *   - isVisible=false (component scrolled out) → pauses rendering
 *   - Cleans up RAF on unmount
 */
export function useDynamicFPS({
  onFrame,
  isVisible = true,
  isPageVisible = true,
  observeRef,
  trackPageVisibility = false,
  fpsActive = 60,
  fpsIdle = 15,
  isActive = true,
  isPaused = false,
}: DynamicFPSOptions) {
  // ── Self-managed visibility: IntersectionObserver + page visibility ──
  // Both default to true so consumers that don't opt in keep exact behaviour.
  const [observedVisible, setObservedVisible] = useState(true);
  const [pageVisibleState, setPageVisibleState] = useState(
    typeof document === "undefined" ? true : document.visibilityState === "visible"
  );

  useEffect(() => {
    const el = observeRef?.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    setObservedVisible(true);
    const io = new IntersectionObserver(
      ([entry]) => setObservedVisible(entry.isIntersecting),
      { threshold: 0 }
    );
    io.observe(el);
    return () => io.disconnect();
  }, [observeRef]);

  useEffect(() => {
    if (!trackPageVisibility || typeof document === "undefined") return;
    const onVis = () => setPageVisibleState(document.visibilityState === "visible");
    onVis();
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, [trackPageVisibility]);

  const effectiveVisible = isVisible && observedVisible;
  const effectivePageVisible = isPageVisible && pageVisibleState;

  // ── Store all changing values in refs so the RAF loop never stalls ──
  const onFrameRef = useRef(onFrame);
  const isActiveRef = useRef(isActive);
  const isPausedRef = useRef(isPaused);
  const isVisibleRef = useRef(effectiveVisible);
  const isPageVisibleRef = useRef(effectivePageVisible);
  const fpsActiveRef = useRef(fpsActive);
  const fpsIdleRef = useRef(fpsIdle);

  onFrameRef.current = onFrame;
  isActiveRef.current = isActive;
  isPausedRef.current = isPaused;
  isVisibleRef.current = effectiveVisible;
  isPageVisibleRef.current = effectivePageVisible;
  fpsActiveRef.current = fpsActive;
  fpsIdleRef.current = fpsIdle;

  // ── RAF state ──
  const rafRef = useRef<number | null>(null);
  const lastFrameTimeRef = useRef<number>(0);

  // ── Stable loop body — reads from refs on each tick ──
  const loop = useCallback((timestamp: number) => {
    if (
      isPausedRef.current ||
      !isVisibleRef.current ||
      !isPageVisibleRef.current
    ) {
      rafRef.current = null;
      return;
    }

    const targetFps = isActiveRef.current
      ? fpsActiveRef.current
      : fpsIdleRef.current;

    if (targetFps > 0) {
      const frameInterval = 1000 / targetFps;
      const delta = timestamp - lastFrameTimeRef.current;
      if (delta >= frameInterval) {
        lastFrameTimeRef.current =
          timestamp - (delta % frameInterval);
        onFrameRef.current(delta);
      }
    }

    rafRef.current = requestAnimationFrame(loop);
  }, []);

  // ── Lifecycle: start/stop loop based on control flags ──
  // Uses the effective flags (caller props ANDed with self-managed observers)
  // so a scrolled-out element or hidden tab cancels the RAF entirely.
  useEffect(() => {
    const shouldRun = !isPaused && effectiveVisible && effectivePageVisible;

    if (shouldRun) {
      if (rafRef.current === null) {
        lastFrameTimeRef.current = performance.now();
        rafRef.current = requestAnimationFrame(loop);
      }
    } else if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }

    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, [isPaused, effectiveVisible, effectivePageVisible, loop]);

  // ── Manual start/stop for imperative control ──
  const start = useCallback(() => {
    if (rafRef.current === null) {
      lastFrameTimeRef.current = performance.now();
      rafRef.current = requestAnimationFrame(loop);
    }
  }, [loop]);

  const stop = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  return { start, stop };
}
