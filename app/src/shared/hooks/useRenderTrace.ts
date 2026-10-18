import { useEffect, useRef } from "react";

let MOUNT_SEQ = 0;

/**
 * Render tracing is ON in dev by default. It is the RCA instrument for the
 * "session panel re-renders / lags" investigation.
 *
 * Disable it (clean perf run) with either:
 *   - the dev URL:  ?trace=0
 *   - the console:  localStorage.setItem("vox.trace", "0")  then reload
 */
export const RENDER_TRACE_ENABLED: boolean = (() => {
  if (!import.meta.env.DEV) return false;
  try {
    if (localStorage.getItem("vox.trace") === "0") return false;
  } catch {
    /* storage unavailable */
  }
  if (typeof window !== "undefined") {
    if (/[?&]trace=0(&|$)/.test(window.location.search)) return false;
  }
  return true;
})();

export interface RenderTraceOptions {
  /** Heartbeat cadence when no `watch` is supplied (default 25 renders). */
  heartbeat?: number;
  /**
   * Values to diff between consecutive renders, with `Object.is` — which is
   * exactly React's bail-out test. A changed array/callback identity listed here
   * is the *reason* the component re-rendered.
   *
   * Use it as: `useRenderTrace("Thing", undefined, { watch: { someArray, onChange } })`.
   * Logs `changed=[someArray]` instead of a bare render count.
   */
  watch?: Record<string, unknown>;
  /**
   * Cap the number of change lines per instance (0 = unlimited, default 0).
   * Set a small cap for list rows so hundreds of rows stay readable.
   */
  maxChangeLogs?: number;
}

/** Keys whose `Object.is` identity differs between two watched snapshots. */
function diffKeys(
  prev: Record<string, unknown>,
  next: Record<string, unknown>
): string[] {
  const changed: string[] = [];
  for (const key of Object.keys(next)) {
    if (!Object.is(prev[key], next[key])) changed.push(key);
  }
  for (const key of Object.keys(prev)) {
    if (!(key in next)) changed.push(`-${key}`);
  }
  return changed;
}

/**
 * DEV-only lifecycle + re-render trace for remount / render-storm RCA.
 *
 * Read one line as:
 *   `[trace] Name#7 MOUNT`
 *   `[trace] Name#7 render#12 changed=[projects,onRename] dt=0.4ms`
 *   `[trace] Name#7 UNMOUNT lifetime=9980.1ms renders=41`
 *
 *   - MOUNT/UNMOUNT with different ids  -> genuine remount.
 *   - rising `render#` under one id     -> re-render storm.
 *   - `changed=[...]`                    -> the prop/state identities that
 *                                          caused that render.
 *   - `dt`                               -> ms since this instance's previous
 *                                          render (dt≈0 = a storm).
 */
export function useRenderTrace(
  name: string,
  detail?: () => Record<string, unknown>,
  options: RenderTraceOptions = {}
): void {
  const enabled = RENDER_TRACE_ENABLED;
  const idRef = useRef(0);
  const renderCountRef = useRef(0);
  const mountedAtRef = useRef(0);
  const lastRenderAtRef = useRef(0);
  const changeLogsRef = useRef(0);
  const watchRef = useRef<Record<string, unknown> | null>(null);
  const detailRef = useRef(detail);
  /** Cleared only after the setup's macrotask, so a same-task cleanup is StrictMode. */
  const settledRef = useRef(false);

  // MOUNT / UNMOUNT once per instance. Runs first so it captures the lifetime
  // before the per-render effect below bumps the counter.
  useEffect(() => {
    if (!enabled) return;
    idRef.current = idRef.current || ++MOUNT_SEQ;
    mountedAtRef.current = performance.now();
    lastRenderAtRef.current = mountedAtRef.current;
    const id = idRef.current;
    // StrictMode re-runs mount effects inside the same macrotask, so a cleanup
    // that runs while this is still false is StrictMode, not a real unmount.
    // A time threshold cannot tell the two apart: with many sibling rows, the
    // console logging alone inflates the measured lifetime past any threshold.
    settledRef.current = false;
    const settleTimer = setTimeout(() => {
      settledRef.current = true;
    }, 0);
    console.info(`[trace] ${name}#${id} MOUNT`, detailRef.current?.() ?? {});
    return () => {
      clearTimeout(settleTimer);
      const lifetime = performance.now() - mountedAtRef.current;
      const tag = settledRef.current ? "UNMOUNT" : "UNMOUNT(strict)";
      console.info(
        `[trace] ${name}#${id} ${tag} lifetime=${lifetime.toFixed(1)}ms renders=${renderCountRef.current}`
      );
    };
  }, [enabled, name]);

  // Per-render bookkeeping. No dependency array: runs after every render, but
  // only ever logs on a watched-identity change, a heartbeat, or mount.
  useEffect(() => {
    if (!enabled) return;
    detailRef.current = detail;
    const count = (renderCountRef.current += 1);
    const now = performance.now();
    const dt = now - lastRenderAtRef.current;
    lastRenderAtRef.current = now;
    const id = idRef.current;
    const heartbeat = options.heartbeat ?? 25;
    const watch = options.watch;

    if (watch) {
      const prev = watchRef.current;
      watchRef.current = watch;
      if (prev) {
        const changed = diffKeys(prev, watch);
        if (changed.length > 0) {
          const cap = options.maxChangeLogs ?? 0;
          if (cap === 0 || changeLogsRef.current < cap) {
            changeLogsRef.current += 1;
            console.info(
              `[trace] ${name}#${id} render#${count} changed=[${changed.join(",")}] dt=${dt.toFixed(1)}ms`
            );
          }
        } else if (count > 1 && count % heartbeat === 0) {
          // Re-rendered with nothing watched changed: the cause is internal
          // state or a context/force-update, not props. Flag it loudly.
          console.info(
            `[trace] ${name}#${id} render#${count} changed=[NONE-watched] dt=${dt.toFixed(1)}ms`,
            detailRef.current?.() ?? {}
          );
        }
      }
      return;
    }

    if (count > 1 && count % heartbeat === 0) {
      console.info(
        `[trace] ${name}#${id} render#${count} dt=${dt.toFixed(1)}ms`,
        detailRef.current?.() ?? {}
      );
    }
  });
}
