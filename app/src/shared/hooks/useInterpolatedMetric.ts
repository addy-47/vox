import { useRef, useState, useEffect } from "react";

/**
 * useInterpolatedMetric
 *
 * Smoothly interpolates a raw numeric value (typically from a 1Hz backend poll)
 * toward its target using an Exponential Moving Average (EMA) running at ~60fps.
 *
 * Formula per frame: smoothed = smoothed * (1 - alpha) + raw * alpha
 *
 * alpha = 0.08 → very smooth, ~2s convergence
 * alpha = 0.15 → balanced, ~1s convergence  ← default
 * alpha = 0.30 → snappy, ~0.5s convergence
 *
 * This eliminates the "stepped" appearance when charts update at backend poll rate.
 * The RAF loop only runs while the hook is mounted and stops cleanly on unmount.
 */
export function useInterpolatedMetric(raw: number, alpha = 0.12): number {
  const smoothedRef = useRef<number>(raw);
  const rawRef = useRef<number>(raw);
  const rafRef = useRef<number>(0);
  const [display, setDisplay] = useState<number>(raw);

  // Keep rawRef in sync without triggering the RAF loop restart
  useEffect(() => {
    rawRef.current = raw;
  }, [raw]);

  useEffect(() => {
    // Seed the smoothed value with the initial raw so there's no startup jump
    smoothedRef.current = rawRef.current;

    const tick = () => {
      const target = rawRef.current;
      const current = smoothedRef.current;
      const next = current + (target - current) * alpha;

      // Only trigger a re-render if the delta is perceptible (> 0.05)
      if (Math.abs(next - current) > 0.05) {
        smoothedRef.current = next;
        setDisplay(next);
      }

      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(rafRef.current);
    };
    // alpha is stable; re-running on alpha change is intentional
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [alpha]);

  return display;
}
