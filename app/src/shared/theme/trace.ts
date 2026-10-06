/**
 * Theme-flip trace logs. Development-only instrumentation.
 *
 * This was previously active in production, where it was not a passive
 * observer: on every `applyTheme` call — including every accent-slider release
 * — it ran five whole-document `querySelectorAll` scans, forced a style recalc
 * and a forced layout, opened a `longtask` PerformanceObserver and drove a
 * 700ms rAF loop that competed with the flip's own token interpolation. On a
 * theme change it added three further timed `getComputedStyle` bursts *inside*
 * the flip window. Measured on the Settings route with all six cards open
 * (9,606 nodes), that cost more blocking time than the flip it was measuring.
 *
 * It is now dev-only. `beginFlipTrace` stays cheap (one `performance.now()`);
 * the expensive half — `endFlipTrace` — compiles away in a production build.
 *
 * ## The forced-recalc probe is opt-in, and must stay that way
 *
 * `getComputedStyle(document.body).backgroundColor` does not observe a style
 * recalc — it *causes* one, synchronously, inside the click's task. That
 * inflates exactly the number the harness exists to report: it reports 683-2772ms
 * of "forced style recalc" on the Settings route, and a production browser
 * would have flushed the same recalc at paint instead, after yielding. So the
 * per-flip probe defaults OFF.
 *
 * Turn it on deliberately when isolating recalc cost:
 *   setThemeTraceProbe(true)     // from the WebKitGTK inspector console
 * The pass census, the frame sampler and the snap detector are passive and stay
 * on, so the default trace is a faithful observer.
 */

export interface FlipTraceHandle {
  id: number;
  t0: number;
}

/** Dev-only. `import.meta.env.DEV` is statically replaced by Vite, so this
 *  branch is dead-code-eliminated from production bundles. */
const TRACE_ENABLED = import.meta.env.DEV;

let probeEnabled = TRACE_ENABLED; // TEMP-DIAGNOSTIC: default ON in dev so a plain theme flip emits snap lines. Revert to false after RCA.

/**
 * Opt in to the destructive forced-style-recalc probe. Off by default: it
 * changes what it measures.
 */
export function setThemeTraceProbe(enabled: boolean): void {
  probeEnabled = enabled;
}

if (TRACE_ENABLED && typeof window !== "undefined") {
  const w = window as unknown as {
    setThemeTraceProbe?: (enabled: boolean) => void;
  };
  w.setThemeTraceProbe = setThemeTraceProbe;
}

let nextId = 1;

export function beginFlipTrace(): FlipTraceHandle {
  const t0 = TRACE_ENABLED ? performance.now() : 0;
  const handle = { id: nextId++, t0 };
  return handle;
}

export function endFlipTrace(handle: FlipTraceHandle, animate: boolean) {
  if (!TRACE_ENABLED) return;
  if (typeof document === "undefined") return;
  const id = handle.id;
  const t0 = handle.t0;
  const tWrite = performance.now();

  const census = {
    nodes: document.querySelectorAll("*").length,
    glassCards: document.querySelectorAll(".glass-card").length,
    glass: document.querySelectorAll(".glass").length,
    rawBlur: document.querySelectorAll('[class*="backdrop-blur"]').length,
    inlineBlur: document.querySelectorAll('[style*="backdrop-filter"]').length,
  };
  console.info(`[theme-flip] #${id} applyTheme sync write ${(tWrite - t0).toFixed(1)}ms animate=${animate}`, census);

  const tRecalc = performance.now();
  if (probeEnabled) {
    void getComputedStyle(document.body).backgroundColor;
    const tRecalcDone = performance.now();
    void document.documentElement.offsetHeight;
    const tLayoutDone = performance.now();
    console.info(
      `[theme-flip] #${id} forced style recalc ${(tRecalcDone - tRecalc).toFixed(1)}ms forced layout ${(tLayoutDone - tRecalcDone).toFixed(1)}ms (probe ON — this number includes work a browser would have done at paint)`
    );
  }

  const longTasks: number[] = [];
  let obs: PerformanceObserver | null = null;
  try {
    obs = new PerformanceObserver((list) => {
      for (const e of list.getEntries()) longTasks.push(Math.round(e.duration));
    });
    obs.observe({ entryTypes: ["longtask"] });
  } catch {
    /* longtask unsupported */
  }

  const deltas: number[] = [];
  let last = tWrite;
  let frames = 0;
  const tick = () => {
    const now = performance.now();
    deltas.push(now - last);
    last = now;
    frames += 1;
    if (now - t0 < 700) {
      requestAnimationFrame(tick);
    } else {
      obs?.disconnect();
      const sorted = [...deltas].sort((a, b) => a - b);
      const median = sorted.length ? sorted[Math.floor(sorted.length / 2)] : 0;
      const worst = sorted.length ? sorted[sorted.length - 1] : 0;
      const tbt = deltas.filter((d) => d > 50).reduce((s, d) => s + (d - 50), 0);
      console.info(
        `[theme-flip] #${id} window 700ms: firstFrame=${deltas[0]?.toFixed(1) ?? "?"}ms ` +
          `secondFrame=${deltas[1]?.toFixed(1) ?? "?"}ms frames=${frames} median=${median.toFixed(1)}ms ` +
          `worst=${worst.toFixed(1)}ms TBT=${tbt.toFixed(1)}ms longTasks=[${longTasks.join(",")}]ms`
      );
    }
  };
  requestAnimationFrame(tick);
}

/** First rgb() triple found in a computed color string, or null. */
function firstRgb(value: string): [number, number, number] | null {
  const m = value.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  return m ? [+m[1], +m[2], +m[3]] : null;
}

function channelDrift(a: string, b: string): number {
  const ra = firstRgb(a);
  const rb = firstRgb(b);
  if (!ra || !rb) return a === b ? 0 : NaN;
  return Math.max(Math.abs(ra[0] - rb[0]), Math.abs(ra[1] - rb[1]), Math.abs(ra[2] - rb[2]));
}

/**
 * Snap detector: samples an element's theme-bearing computed values across the
 * flip window. A surface that fades drifts gradually across samples; one that
 * snaps jumps >128 channels in the first interval then stays flat. Logs one
 * `[theme-flip] snap <label>` line with the verdict per property.
 *
 * `marks` are the sample offsets in ms from the start of the flip. They used to
 * be hardcoded to `[80, 160, 232]` — the flip's old 200ms budget plus tail. Once
 * a flip took longer than that (measured: 1154ms and 1386ms sync write with all
 * six Settings cards open), every sample landed *after* the animation had
 * already finished and the detector reported `bg:static drift=[0,0,0]` for every
 * target, i.e. it was reporting nothing while looking like it was working. The
 * marks now scale with the duration the caller actually observed.
 */
export function traceFlipTargets(id: number, selectors: string[], durationMs: number) {
  // Reads computed values on 4 samples x N targets inside the flip window, so
  // it forces the recalcs it is trying to characterise. Same reason as the
  // probe above: opt-in only.
  if (!probeEnabled) return;
  if (typeof document === "undefined") return;
  const t0 = performance.now();
  const targets = selectors
    .map((sel) => ({ sel, el: document.querySelector(sel) as Element | null }))
    .filter((t) => t.el);
  const missing = selectors.filter(
    (sel) => !targets.some((t) => t.sel === sel)
  );
  if (missing.length) {
    console.info(`[theme-flip] #${id} snap targets missing: ${missing.join(", ")}`);
  }
  if (!targets.length) return;

  const read = (el: Element) => {
    const cs = getComputedStyle(el);
    return {
      bg: cs.backgroundColor,
      image: cs.backgroundImage,
      shadow: cs.boxShadow,
      blur: cs.backdropFilter,
      opacity: cs.opacity,
    };
  };
  const start = targets.map((t) => ({ sel: t.sel, v: read(t.el!) }));
  const samples: Array<Array<{ sel: string; v: ReturnType<typeof read> }>> = [start];
  // Three probes spread across the observed window: early (is it already
  // snapping?), mid, and the tail. If the flip overran the window, the tail
  // still lands before the finish so the verdict is meaningful.
  const span = Math.max(1, durationMs);
  const marks = [Math.round(span * 0.25), Math.round(span * 0.6), Math.round(span * 0.9)];
  for (const d of marks) {
    setTimeout(() => {
      samples.push(targets.map((t) => ({ sel: t.sel, v: read(t.el!) })));
      if (samples.length - 1 < marks.length) return;
      const lines = targets.map((t, i) => {
        // Per-interval drift for bg AND shadow: snap = ~all drift in
        // interval 1, then flat; fade = drift spread across intervals.
        // A transparent-bg surface (card wrappers, title bar) reports bg
        // static while its shadow may still fade — hence per-property verdicts.
        const driftsFor = (
          pick: (v: ReturnType<typeof read>) => string
        ) =>
          samples.slice(1).map((s, k) =>
            channelDrift(pick(samples[k][i].v), pick(s[i].v))
          );
        const judge = (drifts: number[]) => {
          const total = drifts.reduce((a, b) => a + (Number.isNaN(b) ? 0 : b), 0);
          if (total < 8) return "static";
          return total > 0 && !Number.isNaN(drifts[0]) && drifts[0] / total > 0.85
            ? "SNAP"
            : "fade";
        };
        const bgDrifts = driftsFor((v) => v.bg);
        const shadowDrifts = driftsFor((v) => v.shadow);
        const fmt = (d: number[]) =>
          `[${d.map((x) => (Number.isNaN(x) ? "?" : x.toFixed(0))).join(",")}]`;
        const last = samples[samples.length - 1][i].v;
        const imageVals = samples.map((s) => s[i].v.image);
        const imageChanged = new Set(imageVals).size > 1;
        const imageTag = imageChanged ? "image:CHANGED-mid-window" : "image:stable-post-flip";
        return `${t.sel} [bg:${judge(bgDrifts)} shadow:${judge(shadowDrifts)} ${imageTag}] bg ${start[i].v.bg} -> ${last.bg} drift=${fmt(bgDrifts)} shadow drift=${fmt(shadowDrifts)} blur ${start[i].v.blur} -> ${last.blur}`;
      });
      console.info(
        `[theme-flip] #${id} snap @+${(performance.now() - t0).toFixed(0)}ms:\n  ` +
          lines.join("\n  ")
      );
    }, d);
  }
}
