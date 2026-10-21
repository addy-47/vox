/**
 * Temporary RCA tracing. Off unless `localStorage["vox:rca"] === "1"`.
 *
 * Enable in the browser console:
 *   localStorage["vox:rca"] = "1"; location.reload()
 * Disable:
 *   localStorage.removeItem("vox:rca")
 *
 * Delete this module and its call sites once the model-select flicker and the
 * close-hover banner jitter are explained.
 */
const KEY = "vox:rca";

export function rcaEnabled(): boolean {
  try {
    return typeof localStorage !== "undefined" && localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export function rca(scope: string, event: string, data: Record<string, unknown> = {}): void {
  if (!rcaEnabled()) return;
  console.info(`[rca:${scope}] ${event}`, {
    t: Math.round(performance.now()),
    ...data,
  });
}

/** Rect of an element, rounded to whole pixels, for layout-shift comparisons. */
export function rcaRect(el: Element | null | undefined): string {
  if (!el) return "none";
  const r = el.getBoundingClientRect();
  return `${Math.round(r.left)},${Math.round(r.top)} ${Math.round(r.width)}x${Math.round(r.height)}`;
}
