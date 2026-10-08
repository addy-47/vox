/**
 * Frontend capability layer (Android preparation, Batch 5.1).
 *
 * Single module for platform capability checks. Derived from
 * `matchMedia('(pointer: coarse)')` and `navigator.maxTouchPoints`.
 * Desktop behavior is the default; all branches are additive.
 */

export function isCoarsePointer(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  try {
    return window.matchMedia("(pointer: coarse)").matches;
  } catch {
    return false;
  }
}

export function isTouch(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return (
      (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0) ||
      isCoarsePointer()
    );
  } catch {
    return false;
  }
}

export function isDesktop(): boolean {
  return !isCoarsePointer() && !isTouch();
}
