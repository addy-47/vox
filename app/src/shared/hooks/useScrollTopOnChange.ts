import { useEffect, useRef, type RefObject } from "react";
import type Lenis from "lenis";

/**
 * Smooth-scrolls a list container back to top when `key` changes, so a
 * save-gated reorder glides instead of jumping. Skips the first mount and
 * ignores re-renders where the key is unchanged. Prefers the Lenis instance
 * when the container is Lenis-managed, otherwise native smooth scrolling.
 */

const SAVE_GATED_GLIDE_DURATION = 1.6;

function saveGatedGlideEasing(t: number): number {
  if (t >= 1) return 1;
  return 1 - Math.pow(2, -10 * t);
}
export function useScrollTopOnChange(
  containerRef: RefObject<HTMLElement | null>,
  key: string | undefined,
  lenisRef?: RefObject<Lenis | null>,
) {
  const prevKey = useRef<string | undefined>(undefined);
  const mounted = useRef(false);

  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true;
      prevKey.current = key;
      return;
    }
    if (prevKey.current === key) return;
    prevKey.current = key;
    const el = containerRef.current;
    if (!el) return;
    if (lenisRef?.current) {
      // Slow GPU-eased glide with a definite stop. Explicit duration/easing
      // keeps this distinct from faster wheel smoothing.
      lenisRef.current.scrollTo(0, {
        duration: SAVE_GATED_GLIDE_DURATION,
        easing: saveGatedGlideEasing,
        force: true,
      });
    } else {
      el.scrollTo({ top: 0, behavior: "smooth" });
    }
  }, [key, containerRef, lenisRef]);
}
