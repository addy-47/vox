import { useState, useEffect } from 'react';

interface PerfMonitorResult {
  fps: number;
  isJanking: boolean;
}

/**
 * Debug hook that tracks FPS over 1-second windows.
 *
 * Only active when `import.meta.env.DEV` is true or
 * `localStorage.getItem('debug_perf')` is set.
 * In production it returns no-op values to avoid overhead.
 */
export function usePerformanceMonitor(
  _label: string,
): PerfMonitorResult {
  const [fps, setFps] = useState(60);
  const [isJanking, setIsJanking] = useState(false);

  useEffect(() => {
    const isDev =
      typeof window !== 'undefined' &&
      (import.meta.env.DEV ||
        localStorage.getItem('debug_perf') !== null);

    if (!isDev) return;

    let rafId: number;
    let lastTime = performance.now();
    let frames = 0;
    let jankCount = 0;

    const check = (timestamp: number) => {
      frames++;

      const elapsed = timestamp - lastTime;
      if (elapsed >= 1000) {
        const currentFps = Math.round((frames * 1000) / elapsed);
        setFps(currentFps);

        if (currentFps < 30) {
          jankCount++;
          setIsJanking(jankCount > 3);
        } else {
          jankCount = 0;
          setIsJanking(false);
        }

        frames = 0;
        lastTime = timestamp;
      }

      rafId = requestAnimationFrame(check);
    };

    rafId = requestAnimationFrame(check);
    return () => cancelAnimationFrame(rafId);
  }, [_label]);

  return { fps, isJanking };
}
