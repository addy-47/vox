import { useRef, useEffect, useCallback, useState } from 'react';

interface DynamicFPSOptions {
  /** Callback receives deltaTime in ms since last non-skipped frame */
  onFrame: (deltaTime: number) => void;
  /** Whether the component is visible (IntersectionObserver) */
  isVisible?: boolean;
  /** Whether the page is visible (document.visibilityState) */
  isPageVisible?: boolean;
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
  fpsActive = 60,
  fpsIdle = 15,
  isActive = true,
  isPaused = false,
}: DynamicFPSOptions) {
  // ── Store all changing values in refs so the RAF loop never stalls ──
  const onFrameRef = useRef(onFrame);
  const isActiveRef = useRef(isActive);
  const isPausedRef = useRef(isPaused);
  const isVisibleRef = useRef(isVisible);
  const isPageVisibleRef = useRef(isPageVisible);
  const fpsActiveRef = useRef(fpsActive);
  const fpsIdleRef = useRef(fpsIdle);

  onFrameRef.current = onFrame;
  isActiveRef.current = isActive;
  isPausedRef.current = isPaused;
  isVisibleRef.current = isVisible;
  isPageVisibleRef.current = isPageVisible;
  fpsActiveRef.current = fpsActive;
  fpsIdleRef.current = fpsIdle;

  // ── RAF state ──
  const rafRef = useRef<number | null>(null);
  const lastFrameTimeRef = useRef<number>(0);
  const fpsCounterRef = useRef({ frames: 0, lastSecond: 0, fps: 60 });
  const [debugFps, setDebugFps] = useState(60);

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

    // FPS counter (1-second windows)
    fpsCounterRef.current.frames++;
    if (timestamp - fpsCounterRef.current.lastSecond >= 1000) {
      setDebugFps(fpsCounterRef.current.frames);
      fpsCounterRef.current.fps = fpsCounterRef.current.frames;
      fpsCounterRef.current.frames = 0;
      fpsCounterRef.current.lastSecond = timestamp;
    }

    rafRef.current = requestAnimationFrame(loop);
  }, []);

  // ── Lifecycle: start/stop loop based on control flags ──
  useEffect(() => {
    const shouldRun = !isPaused && isVisible && isPageVisible;

    if (shouldRun) {
      if (rafRef.current === null) {
        lastFrameTimeRef.current = performance.now();
        fpsCounterRef.current = {
          frames: 0,
          lastSecond: performance.now(),
          fps: 60,
        };
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
  }, [isPaused, isVisible, isPageVisible, loop]);

  // ── Manual start/stop for imperative control ──
  const start = useCallback(() => {
    if (rafRef.current === null) {
      lastFrameTimeRef.current = performance.now();
      fpsCounterRef.current = {
        frames: 0,
        lastSecond: performance.now(),
        fps: 60,
      };
      rafRef.current = requestAnimationFrame(loop);
    }
  }, [loop]);

  const stop = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  return { start, stop, fps: debugFps };
}
