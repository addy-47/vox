import Lenis, { type LenisOptions } from "lenis";
import { isCoarsePointer } from "@/lib/capabilities";

type ContainedLenisOptions = Omit<LenisOptions, "wrapper" | "content" | "eventsTarget">;

/**
 * Canonical smooth-scroll standard for Vox (SSOT).
 *
 * Industry pattern (per Lenis docs):
 * - Wheel input is smoothed with inertial easing (velocity decays, glide
 *   stops slowly — the "dynamic to touch intensity" feel). Touch/trackpad
 *   momentum stays native (`smoothTouch` off).
 * - Exactly ONE Lenis instance per scroll container, created via
 *   `useLenisScrollContainer`. Never `new Lenis` ad-hoc in components.
 * - Nested scrollers opt out of the parent via `overscroll-behavior:
 *   contain` (see SMOOTH_SCROLL_CONTAINER_CLASS) so wheel chaining never
 *   locks the inner list.
 * - CSS scroll-snap is INCOMPATIBLE with Lenis virtual scroll — never combine
 *   `snap-*` classes with a Lenis-managed container.
 */

export const SMOOTH_SCROLL_DURATION = 0.85;

/** Canonical Tailwind classes for every Lenis-managed scroll container. */
export const SMOOTH_SCROLL_CONTAINER_CLASS =
  "overflow-y-auto overscroll-contain custom-scrollbar";

/**
 * Creates a scoped Lenis instance for a contained scroller, or `null` on
 * coarse-pointer (touch) devices.
 *
 * Lenis only smooths wheel input here (smoothTouch is off by default), so on
 * phones/tablets the instance contributes nothing but its own autoRaf loop —
 * pure CPU/battery cost against native momentum scrolling. Callers fall back
 * to native scroll; `useScrollTopOnChange` already handles a null lenisRef.
 *
 * Correct wrapper/content split: the scrollable element is the wrapper and
 * its first child is the content Lenis transforms. Passing the same element
 * for both (the old ad-hoc pattern) breaks height measurement and locks
 * scrolling whenever content height changes (filtering, pagination).
 */
export function createContainedLenis(
  el: HTMLElement,
  options: ContainedLenisOptions = {},
): Lenis | null {
  if (isCoarsePointer()) return null;
  const content = (el.firstElementChild as HTMLElement | null) ?? el;
  if (import.meta.env.DEV && el.children.length !== 1) {
    console.warn(
      "[Lenis] Scroll wrapper should have exactly 1 child content wrapper; found:",
      el.children.length,
      el,
    );
  }
  return new Lenis({
    wrapper: el,
    content,
    eventsTarget: el,
    smoothWheel: true,
    autoRaf: true,
    duration: SMOOTH_SCROLL_DURATION,
    ...options,
  });
}
