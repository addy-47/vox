import { useEffect, useRef, type RefObject } from "react";
import Lenis from "lenis";
import { createContainedLenis } from "@/shared/lib/lenisScroll";

export interface LenisScrollContainer<T extends HTMLElement> {
  containerRef: RefObject<T | null>;
  lenisRef: RefObject<Lenis | null>;
}

/**
 * Creates one GPU-eased Lenis instance for a contained settings list. The
 * same container/lenis refs drive both smooth wheel input and the slow
 * save-gated glide back to top, so post-save reorders never jump.
 * No-op on coarse-pointer devices (native scroll; see createContainedLenis).
 */
export function useLenisScrollContainer<T extends HTMLElement = HTMLDivElement>(): LenisScrollContainer<T> {
  const containerRef = useRef<T | null>(null);
  const lenisRef = useRef<Lenis | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return undefined;

    const lenis = createContainedLenis(el, { duration: 0.85 });
    if (!lenis) return undefined;
    lenisRef.current = lenis;

    return () => {
      lenisRef.current = null;
      lenis.destroy();
    };
  }, []);

  return { containerRef, lenisRef };
}
