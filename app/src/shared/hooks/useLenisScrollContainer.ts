import { useEffect, useRef, type RefObject } from "react";
import Lenis from "lenis";
import { createContainedLenis, SMOOTH_SCROLL_DURATION } from "@/shared/lib/lenisScroll";

export interface LenisScrollContainer<T extends HTMLElement> {
  containerRef: RefObject<T | null>;
  lenisRef: RefObject<Lenis | null>;
  /** Imperatively reset scroll to top (Lenis-aware, instant). */
  scrollToTop: () => void;
}

/**
 * Canonical hook for smooth inertial scrolling (SSOT).
 *
 * Creates one GPU-eased Lenis instance per contained list. The same
 * container/lenis refs drive both smooth wheel input (velocity-decaying
 * glide) and the slow save-gated glide back to top, so post-save reorders
 * never jump. No-op on coarse-pointer devices (native momentum scroll).
 *
 * Rules:
 * - Do NOT combine the managed container with CSS `snap-*` classes.
 * - Do NOT nest two Lenis-managed scrollers; the outer must be
 *   `overflow-hidden` and only the inner scrolls.
 */
export function useLenisScrollContainer<T extends HTMLElement = HTMLDivElement>(
  duration: number = SMOOTH_SCROLL_DURATION,
): LenisScrollContainer<T> {
  const containerRef = useRef<T | null>(null);
  const lenisRef = useRef<Lenis | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return undefined;

    const lenis = createContainedLenis(el, { duration });
    if (!lenis) return undefined;
    lenisRef.current = lenis;

    return () => {
      lenisRef.current = null;
      lenis.destroy();
    };
  }, [duration]);

  const scrollToTopRef = useRef(() => {
    const el = containerRef.current;
    if (!el) return;
    if (lenisRef.current) {
      lenisRef.current.scrollTo(0, { immediate: true });
    } else {
      el.scrollTop = 0;
    }
  });

  return {
    containerRef,
    lenisRef,
    scrollToTop: () => scrollToTopRef.current(),
  };
}
