import { useState, useEffect, useRef, useCallback } from "react";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

export function useMonitoringMetrics() {
  const [history, setHistory] = useState<(RuntimeSnapshot & { localTime: number })[]>([]);
  const [engineToggling, setEngineToggling] = useState(false);

  const cpuTextRef = useRef<HTMLSpanElement>(null);
  const cpuBarRef = useRef<HTMLDivElement>(null);
  const ramTextRef = useRef<HTMLSpanElement>(null);
  const ramBarRef = useRef<HTMLDivElement>(null);

  const latest = history[history.length - 1] ?? null;
  const latestRef = useRef<RuntimeSnapshot | null>(latest);
  latestRef.current = latest;

  // Background Polling Loop
  useEffect(() => {
    const poll = async () => {
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
      }
    };

    poll();
    const id = setInterval(poll, POLL_MS);
    return () => clearInterval(id);
  }, []);

  // Direct DOM Interpolation Loop (EMA) at 60fps
  useEffect(() => {
    let curCpu = 0;
    let curRam = 0;
    let rafId = 0;

    if (latestRef.current) {
      curCpu = latestRef.current.vox_cpu_usage;
      curRam = latestRef.current.vox_ram_mb;
    }

    const tick = () => {
      const snap = latestRef.current;
      if (snap) {
        const targetCpu = snap.vox_cpu_usage;
        const targetRam = snap.vox_ram_mb;

        curCpu += (targetCpu - curCpu) * 0.12;
        curRam += (targetRam - curRam) * 0.12;

        if (cpuTextRef.current) {
          cpuTextRef.current.textContent = `${curCpu.toFixed(1)}%`;
        }
        if (cpuBarRef.current) {
          cpuBarRef.current.style.width = `${Math.min(100, Math.max(0, curCpu))}%`;
        }
        if (ramTextRef.current) {
          const ramGb = curRam / 1024;
          ramTextRef.current.textContent = `${ramGb.toFixed(2)} GB`;
        }
        if (ramBarRef.current) {
          const pct = Math.min(100, Math.max(0, (curRam / snap.total_ram_mb) * 100));
          ramBarRef.current.style.width = `${pct}%`;
        }
      }
      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

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
    cpuTextRef,
    cpuBarRef,
    ramTextRef,
    ramBarRef,
    formatLatency,
  };
}
