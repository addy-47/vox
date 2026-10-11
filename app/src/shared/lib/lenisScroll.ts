import Lenis, { type LenisOptions } from "lenis";
import { isCoarsePointer } from "@/lib/capabilities";

type ContainedLenisOptions = Omit<LenisOptions, "wrapper" | "content" | "eventsTarget">;

/**
 * Creates a scoped Lenis instance for a contained scroller, or `null` on
 * coarse-pointer (touch) devices.
 *
 * Lenis only smooths wheel input here (smoothTouch is off by default), so on
 * phones/tablets the instance contributes nothing but its own autoRaf loop —
 * pure CPU/battery cost against native momentum scrolling. Callers fall back
 * to native scroll; `useScrollTopOnChange` already handles a null lenisRef.
 */
export function createContainedLenis(
  el: HTMLElement,
  options: ContainedLenisOptions,
): Lenis | null {
  if (isCoarsePointer()) return null;
  return new Lenis({
    wrapper: el,
    content: el,
    eventsTarget: el,
    smoothWheel: true,
    autoRaf: true,
    ...options,
  });
}