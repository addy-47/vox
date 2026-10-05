import { useEffect, useRef, type RefObject } from "react";
import Lenis from "lenis";

export interface LenisScrollContainer<T extends HTMLElement> {
  containerRef: RefObject<T | null>;
  lenisRef: RefObject<Lenis | null>;
}

/**
 * Creates one GPU-eased Lenis instance for a contained settings list. The
 * same container/lenis refs drive both smooth wheel input and the slow
 * save-gated glide back to top, so post-save reorders never jump.
 */
export function useLenisScrollContainer<T extends HTMLElement = HTMLDivElement>(): LenisScrollContainer<T> {
  const containerRef = useRef<T | null>(null);
  const lenisRef = useRef<Lenis | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return undefined;

    const lenis = new Lenis({
      wrapper: el,
      content: el,
      eventsTarget: el,
      smoothWheel: true,
      autoRaf: true,
      duration: 0.85,
    });
    lenisRef.current = lenis;

    return () => {
      lenisRef.current = null;
      lenis.destroy();
    };
  }, []);

  return { containerRef, lenisRef };
}
