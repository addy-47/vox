import { useState, useEffect, useRef, useCallback } from "react";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

export function useMonitoringMetrics(enabled = true) {
  const [history, setHistory] = useState<(RuntimeSnapshot & { localTime: number })[]>([]);
  const [engineToggling, setEngineToggling] = useState(false);

  const latest = history[history.length - 1] ?? null;
  const latestRef = useRef<RuntimeSnapshot | null>(latest);
  latestRef.current = latest;

  const inFlightRef = useRef(false);

  // Background Polling Loop (only while monitoring is visible)
  useEffect(() => {
    if (!enabled) return;

    const poll = async () => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      try {
        const snap = await getRuntimeSnapshot();
        if (snap) {
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

    poll();
    const id = setInterval(poll, POLL_MS);
    return () => clearInterval(id);
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