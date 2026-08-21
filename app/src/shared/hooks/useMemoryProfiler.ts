import { useState, useEffect, useRef, useCallback } from "react";
import { useLocation } from "react-router-dom";
import {
  getProfilerSnapshot,
  recordMemoryProfileEvent,
  sampleJSHeap,
  sampleDOMStats,
  sampleCSSIndicators,
  type ProfilerSnapshot,
  type JSHeapSample,
  type DOMSample,
  type CSSIndicatorsSample,
} from "@/services/memoryProfilerService";
import { useMemoryProfilerContext } from "@/shared/context/MemoryProfilerContext";

export interface PageMemoryRecord {
  route: string;
  mountedAt: number;
  unmountedAt?: number;
  baseline: ProfilerSnapshot | null;
  peak: ProfilerSnapshot | null;
  current: ProfilerSnapshot | null;
  retained: ProfilerSnapshot | null;
  retainedDeltaMb: number | null;
  peakDeltaMb: number | null;
  activeComponentsOnMount: string[];
}

export function useMemoryProfiler(enabled = true) {
  const location = useLocation();

  const currentRoute = location.pathname;
  const { componentTraces } = useMemoryProfilerContext();

  const [latestSnapshot, setLatestSnapshot] = useState<ProfilerSnapshot | null>(null);
  const [history, setHistory] = useState<ProfilerSnapshot[]>([]);
  const [pageRecords, setPageRecords] = useState<Record<string, PageMemoryRecord>>({});
  const [jsHeap, setJsHeap] = useState<JSHeapSample>(sampleJSHeap());
  const [domStats, setDomStats] = useState<DOMSample>(sampleDOMStats());
  const [cssStats, setCssStats] = useState<CSSIndicatorsSample>(sampleCSSIndicators());
  const [isSampling, setIsSampling] = useState(false);
  const [lastManualSnapshot, setLastManualSnapshot] = useState<{
    timestampMs: number;
    filename: string;
    totalRamMb: number;
  } | null>(null);

  const pageRecordsRef = useRef<Record<string, PageMemoryRecord>>({});
  pageRecordsRef.current = pageRecords;

  const currentRouteRef = useRef(currentRoute);
  currentRouteRef.current = currentRoute;

  const componentTracesRef = useRef(componentTraces);
  componentTracesRef.current = componentTraces;

  const inFlightRef = useRef(false);

  const captureSnapshot = useCallback(
    async (options?: { isManual?: boolean }): Promise<ProfilerSnapshot | null> => {
      if (inFlightRef.current) return null;
      inFlightRef.current = true;
      setIsSampling(true);
      try {
        const snap = await getProfilerSnapshot();
        setLatestSnapshot(snap);
        setHistory((prev) => {
          const next = [...prev, snap];
          return next.length > 60 ? next.slice(next.length - 60) : next;
        });

        // Probe browser metrics
        const jsSample = sampleJSHeap();
        const domSample = sampleDOMStats();
        const cssSample = sampleCSSIndicators();
        setJsHeap(jsSample);
        setDomStats(domSample);
        setCssStats(cssSample);

        // Update current route metrics in pageRecords
        const r = currentRouteRef.current;
        const existing = pageRecordsRef.current[r];
        const activeComps = Object.keys(componentTracesRef.current).filter(
          (k) => componentTracesRef.current[k].activeInstances > 0
        );
        const domCount = domSample.nodeCount || document.querySelectorAll("*").length;
        const fontCount = domSample.fontFaceCount || (document.fonts ? document.fonts.size : 0);

        const currentTotal = snap.total_vox_ram_mb;
        const baselineTotal = existing?.baseline ? existing.baseline.total_vox_ram_mb : currentTotal;
        const peakTotal = Math.max(existing?.peak ? existing.peak.total_vox_ram_mb : 0, currentTotal);
        const isNewPeak = !existing?.peak || currentTotal > existing.peak.total_vox_ram_mb;
        const updatedPeak = isNewPeak ? snap : (existing?.peak ?? snap);
        const peakDelta = Math.round((peakTotal - baselineTotal) * 100) / 100;

        const updated: PageMemoryRecord = {
          route: r,
          mountedAt: existing?.mountedAt ?? performance.now(),
          baseline: existing?.baseline ?? snap,
          current: snap,
          peak: updatedPeak,
          retained: existing?.retained ?? null,
          retainedDeltaMb: existing?.retainedDeltaMb ?? null,
          peakDeltaMb: peakDelta,
          activeComponentsOnMount: existing?.activeComponentsOnMount ?? activeComps,
        };

        setPageRecords((prev) => ({ ...prev, [r]: updated }));

        // Record snapshot event to backend when triggered manually or upon new peak
        if (options?.isManual) {
          const now = Date.now();
          const cleanRoute = r.replace(/^\/+|\/+$/g, "").replace(/\//g, "_") || "home";
          const filename = `${Math.floor(now / 1000)}-${cleanRoute}.jsonl`;
          setLastManualSnapshot({
            timestampMs: now,
            filename,
            totalRamMb: currentTotal,
          });
          console.info(`[MemoryProfiler] Manual snapshot captured (${currentTotal.toFixed(1)} MB) -> temp/${filename}`, snap);

          await recordMemoryProfileEvent({
            route: r,
            event_type: "snapshot",
            baseline_ram_mb: baselineTotal,
            current_ram_mb: currentTotal,
            peak_ram_mb: peakTotal,
            peak_delta_mb: peakDelta,
            retained_ram_mb: null,
            retained_delta_mb: null,
            main_webview_ram_mb: snap.main_webview_ram_mb,
            tray_webview_ram_mb: snap.tray_webview_ram_mb,
            active_components: activeComps,
            dom_node_count: domCount,
            font_face_count: fontCount,
            timestamp_ms: now,
            process_tree: snap.process_tree,
          });

          // Ensure visual feedback on button
          await new Promise((res) => setTimeout(res, 350));
        } else if (existing && existing.baseline && isNewPeak && peakDelta > 5) {
          recordMemoryProfileEvent({
            route: r,
            event_type: "peak",
            baseline_ram_mb: baselineTotal,
            current_ram_mb: currentTotal,
            peak_ram_mb: peakTotal,
            peak_delta_mb: peakDelta,
            retained_ram_mb: null,
            retained_delta_mb: null,
            main_webview_ram_mb: snap.main_webview_ram_mb,
            tray_webview_ram_mb: snap.tray_webview_ram_mb,
            active_components: activeComps,
            dom_node_count: domCount,
            font_face_count: fontCount,
            timestamp_ms: Date.now(),
            process_tree: snap.process_tree,
          });
        }

        return snap;
      } catch (err) {
        console.warn("[MemoryProfiler] Snapshot capture failed:", err);
        return null;
      } finally {
        inFlightRef.current = false;
        setIsSampling(false);
      }
    },
    []
  );

  // ─── Track Route Changes (Baseline -> Peak -> Retained) ──────────────────────
  useEffect(() => {
    if (!enabled) return;
    const route = location.pathname;

    const activeComponents = Object.keys(componentTracesRef.current).filter(
      (k) => componentTracesRef.current[k].activeInstances > 0
    );

    // 1. Capture Route Baseline
    (async () => {
      const snap = await captureSnapshot();
      if (!snap) return;

      setPageRecords((prev) => {
        const prevRec = prev[route];
        return {
          ...prev,
          [route]: {
            route,
            mountedAt: performance.now(),
            baseline: snap,
            peak: prevRec?.peak || snap,
            current: snap,
            retained: prevRec?.retained || null,
            retainedDeltaMb: prevRec?.retainedDeltaMb || null,
            peakDeltaMb: 0,
            activeComponentsOnMount: activeComponents,
          },
        };
      });

      // Always record the mount event so we have an empirical baseline
      recordMemoryProfileEvent({
        route,
        event_type: "mount",
        baseline_ram_mb: snap.total_vox_ram_mb,
        current_ram_mb: snap.total_vox_ram_mb,
        peak_ram_mb: snap.total_vox_ram_mb,
        peak_delta_mb: 0,
        retained_ram_mb: null,
        retained_delta_mb: null,
        main_webview_ram_mb: snap.main_webview_ram_mb,
        tray_webview_ram_mb: snap.tray_webview_ram_mb,
        active_components: activeComponents,
        dom_node_count: document.querySelectorAll("*").length,
        font_face_count: document.fonts ? document.fonts.size : 0,
        timestamp_ms: Date.now(),
      });
    })();

    // 2. Cleanup function on route unmount: wait 2.5s for GC and capture Retained RAM
    return () => {
      const unmountedRoute = route;
      setTimeout(async () => {
        try {
          const retainedSnap = await getProfilerSnapshot();
          if (retainedSnap) {
            let delta = 0;
            setPageRecords((prev) => {
              const rec = prev[unmountedRoute];
              const baselineMb = rec?.baseline?.total_vox_ram_mb ?? retainedSnap.total_vox_ram_mb;
              delta = Math.round((retainedSnap.total_vox_ram_mb - baselineMb) * 100) / 100;
              return {
                ...prev,
                [unmountedRoute]: {
                  ...(rec || {
                    route: unmountedRoute,
                    mountedAt: performance.now(),
                    baseline: retainedSnap,
                    peak: retainedSnap,
                    current: retainedSnap,
                    peakDeltaMb: 0,
                    activeComponentsOnMount: [],
                  }),
                  unmountedAt: performance.now(),
                  retained: retainedSnap,
                  retainedDeltaMb: delta,
                },
              };
            });

            const rec = pageRecordsRef.current[unmountedRoute];
            const baselineTotal = rec?.baseline?.total_vox_ram_mb;
            const computedDelta = baselineTotal != null ? Math.round((retainedSnap.total_vox_ram_mb - baselineTotal) * 100) / 100 : 0.0;

            recordMemoryProfileEvent({
              route: unmountedRoute,
              event_type: "retained",
              baseline_ram_mb: baselineTotal || retainedSnap.total_vox_ram_mb,
              current_ram_mb: retainedSnap.total_vox_ram_mb,
              peak_ram_mb: rec?.peak?.total_vox_ram_mb || null,
              peak_delta_mb: rec?.peakDeltaMb || null,
              retained_ram_mb: retainedSnap.total_vox_ram_mb,
              retained_delta_mb: computedDelta,
              main_webview_ram_mb: retainedSnap.main_webview_ram_mb,
              tray_webview_ram_mb: retainedSnap.tray_webview_ram_mb,
              active_components: Object.keys(componentTracesRef.current).filter(
                (k) => componentTracesRef.current[k].activeInstances > 0
              ),
              dom_node_count: document.querySelectorAll("*").length,
              font_face_count: document.fonts ? document.fonts.size : 0,
              timestamp_ms: Date.now(),
            });
          }
        } catch {
          // best-effort retained measurement
        }
      }, 2500);
    };
  }, [location.pathname, enabled, captureSnapshot]);

  // ─── Initial Snapshot on Open (No periodic auto-polling; manual snapshots on-demand) ─────
  useEffect(() => {
    if (!enabled) return;
    captureSnapshot();
  }, [enabled, captureSnapshot]);


  // ─── Diagnostic Time-Series Recorder (5s) ───────────────────────────────────
  // Writes a "poll" event every 5 seconds. Disabled by default, can be toggled on when profiling.
  const ENABLE_DIAGNOSTIC_POLL = false;

  useEffect(() => {
    if (!enabled || !ENABLE_DIAGNOSTIC_POLL) return;

    const DIAG_INTERVAL_MS = 5000;

    const diagPoll = async () => {
      if (document.hidden) return;
      try {
        const snap = await getProfilerSnapshot();
        if (!snap) return;
        const route = currentRouteRef.current;
        const rec = pageRecordsRef.current[route];
        await recordMemoryProfileEvent({
          route,
          event_type: "poll",
          baseline_ram_mb: rec?.baseline?.total_vox_ram_mb ?? null,
          current_ram_mb: snap.total_vox_ram_mb,
          peak_ram_mb: rec?.peak?.total_vox_ram_mb ?? null,
          peak_delta_mb: rec?.peakDeltaMb ?? null,
          retained_ram_mb: null,
          retained_delta_mb: null,
          main_webview_ram_mb: snap.main_webview_ram_mb,
          tray_webview_ram_mb: snap.tray_webview_ram_mb,
          active_components: Object.keys(componentTracesRef.current).filter(
            (k) => componentTracesRef.current[k].activeInstances > 0
          ),
          dom_node_count: document.querySelectorAll("*").length,
          font_face_count: document.fonts ? document.fonts.size : 0,
          timestamp_ms: Date.now(),
        });
      } catch {
        // best-effort diagnostic poll
      }
    };

    // Stagger by 2.5s so poll events don't collide with the UI refresh IPC call
    const timerId = setTimeout(() => {
      diagPoll();
      const id = setInterval(diagPoll, DIAG_INTERVAL_MS);
      return () => clearInterval(id);
    }, 2500);

    return () => clearTimeout(timerId);
  }, [enabled, captureSnapshot]);

  const clearHistory = useCallback(() => {
    setHistory([]);
    setPageRecords({});
  }, []);

  return {
    latestSnapshot,
    history,
    pageRecords,
    jsHeap,
    domStats,
    cssStats,
    isSampling,
    currentRoute,
    lastManualSnapshot,
    captureSnapshot,
    clearHistory,
  };
}
