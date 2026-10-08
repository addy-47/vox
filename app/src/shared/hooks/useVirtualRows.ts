import { useCallback, useEffect, useRef, useState } from "react";

export interface VirtualRowWindow {
  start: number;
  end: number;
  topPad: number;
  bottomPad: number;
  /** Stable per-index ref callback (cached map — never inline in `.map()`). */
  itemRef: (index: number) => (el: HTMLDivElement | null) => void;
}

interface UseVirtualRowsOptions {
  /** Fallback row height before a row is measured. */
  estimate?: number;
  /** Extra rows rendered above/below the viewport. */
  overscan?: number;
  /** Lists at or below this length render fully with zero spacers. */
  threshold?: number;
}

/**
 * Variable-height row windowing for long session lists. Row heights are
 * measured on mount into a per-index cache (estimated until measured), so
 * mixed one-line/two-line rows keep a stable scrollbar. The scroll container
 * is resolved via the closest `[data-session-scroll]` ancestor, so nested
 * group lists share the panel's single scroll view without prop plumbing.
 */
export function useVirtualRows(
  wrapperRef: React.RefObject<HTMLDivElement | null>,
  count: number,
  { estimate = 52, overscan = 8, threshold = 40 }: UseVirtualRowsOptions = {}
): VirtualRowWindow {
  const heightsRef = useRef<Map<number, number>>(new Map());
  const refCacheRef = useRef<Map<number, (el: HTMLDivElement | null) => void>>(new Map());
  const [version, setVersion] = useState(0);
  const versionRef = useRef(0);
  const bump = useCallback(() => {
    versionRef.current += 1;
    setVersion(versionRef.current);
  }, []);

  const itemRef = useCallback(
    (index: number) => {
      let fn = refCacheRef.current.get(index);
      if (!fn) {
        fn = (el: HTMLDivElement | null) => {
          const prev = heightsRef.current.get(index);
          const next = el ? el.offsetHeight : undefined;
          if (next && next !== prev) {
            heightsRef.current.set(index, next);
            bump();
          } else if (!el && prev !== undefined) {
            heightsRef.current.delete(index);
          }
        };
        refCacheRef.current.set(index, fn);
      }
      return fn;
    },
    [bump]
  );

  useEffect(() => {
    // Index-keyed caches must not leak across different lists reusing one hook.
    heightsRef.current.clear();
    refCacheRef.current.clear();
  }, [count]);

  useEffect(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper || count <= threshold) return;
    const scroller = wrapper.closest("[data-session-scroll]") as HTMLElement | null;
    if (!scroller) return;

    let raf = 0;
    const update = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => bump());
    };
    scroller.addEventListener("scroll", update, { passive: true });
    const ro = new ResizeObserver(update);
    ro.observe(scroller);
    update();
    return () => {
      cancelAnimationFrame(raf);
      scroller.removeEventListener("scroll", update);
      ro.disconnect();
    };
  }, [wrapperRef, count, threshold, bump]);

  if (count <= threshold) {
    return { start: 0, end: count, topPad: 0, bottomPad: 0, itemRef };
  }

  // Cumulative offsets from the measurement cache (estimate until measured).
  const offsets: number[] = new Array(count + 1);
  offsets[0] = 0;
  for (let i = 0; i < count; i++) {
    offsets[i + 1] = offsets[i] + (heightsRef.current.get(i) ?? estimate);
  }

  const wrapper = wrapperRef.current;
  const scroller = wrapper?.closest("[data-session-scroll]") as HTMLElement | null;
  let viewTop = 0;
  let viewBottom = offsets[count];
  if (wrapper && scroller) {
    const sc = scroller.getBoundingClientRect();
    const wr = wrapper.getBoundingClientRect();
    viewTop = sc.top - wr.top;
    viewBottom = viewTop + sc.height;
  }

  let start = 0;
  while (start < count && offsets[start + 1] < viewTop) start++;
  let end = start;
  while (end < count && offsets[end] < viewBottom) end++;
  start = Math.max(0, start - overscan);
  end = Math.min(count, end + overscan);

  void version;
  return { start, end, topPad: offsets[start], bottomPad: offsets[count] - offsets[end], itemRef };
}
