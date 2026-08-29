import { useState, useEffect, useRef, useCallback } from "react";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

export function useMonitoringMetrics(enabled = true) {
  const [history, setHistory] = useState<(RuntimeSnapshot & { localTime: number })[]>([]);
  const [engineToggling, setEngineToggling] = useState(false);

  const latest = history[history.length - 1] ?? null;
  const latestRef = useRef<RuntimeSnapshot | null>(latest);

  useEffect(() => {
    latestRef.current = latest;
  }, [latest]);

  const inFlightRef = useRef(false);

  // Background Polling Loop (only while monitoring is visible and window is focused/visible)
  useEffect(() => {
    if (!enabled) return;

    let intervalId: ReturnType<typeof setInterval> | null = null;
    let cancelled = false;

    const poll = async () => {
      if (cancelled || document.hidden || inFlightRef.current) return;
      inFlightRef.current = true;
      try {
        const snap = await getRuntimeSnapshot();
        if (snap && !cancelled) {
          setHistory((prev) => {
            const next = [...prev, { ...snap, localTime: performance.now() }];
            return next.length > MAX_SAMPLES ? next.slice(next.length - MAX_SAMPLES) : next;
          });
        }
      } catch {
        // silent
      } finally {
        inFlightRef.current = false;
      }
    };

    const start = () => {
      if (intervalId) clearInterval(intervalId);
      poll();
      intervalId = setInterval(poll, POLL_MS);
    };

    const stop = () => {
      if (intervalId) {
        clearInterval(intervalId);
        intervalId = null;
      }
    };

    const onVisibilityChange = () => {
      if (document.hidden) {
        stop();
      } else {
        start();
      }
    };

    if (!document.hidden) {
      start();
    }

    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      cancelled = true;
      stop();
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [enabled]);

  const formatLatency = useCallback((ms: number | null) => {
    if (ms === null) return "--";
    if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
    return `${ms}ms`;
  }, []);

  return {
    history,
    latest,
    engineToggling,
    setEngineToggling,
    formatLatency,
  };
}