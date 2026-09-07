import { useState, useEffect, useRef } from "react";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";

const POLL_INTERVAL_MS = 30000;

let sharedSnapshot: RuntimeSnapshot | null = null;
let sharedIntervalId: ReturnType<typeof setInterval> | null = null;
let sharedListeners = new Set<(snap: RuntimeSnapshot | null) => void>();
let isVisible = true;

function startSharedPolling(): void {
  if (sharedIntervalId !== null) return;
  const poll = async () => {
    if (document.hidden) return;
    try {
      const snap = await getRuntimeSnapshot();
      if (snap) {
        sharedSnapshot = snap;
      }
    } catch {}
    sharedListeners.forEach((fn) => {
      try { fn(sharedSnapshot); } catch {}
    });
  };
  poll();
  sharedIntervalId = setInterval(poll, POLL_INTERVAL_MS);
  document.addEventListener("visibilitychange", handleVisibility);
}

function stopSharedPolling(): void {
  if (sharedIntervalId !== null) {
    clearInterval(sharedIntervalId);
    sharedIntervalId = null;
  }
  document.removeEventListener("visibilitychange", handleVisibility);
}

function handleVisibility(): void {
  isVisible = !document.hidden;
  if (isVisible) {
    startSharedPolling();
  } else {
    stopSharedPolling();
  }
}

export function useRuntimeSnapshot(enabled: boolean = true): RuntimeSnapshot | null {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot | null>(sharedSnapshot);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    const listener = (snap: RuntimeSnapshot | null) => {
      if (mounted.current) setSnapshot(snap);
    };
    sharedListeners.add(listener);

    if (enabled && sharedIntervalId === null && !document.hidden) {
      startSharedPolling();
    } else if (enabled && sharedSnapshot !== null) {
      setSnapshot(sharedSnapshot);
    }

    return () => {
      mounted.current = false;
      sharedListeners.delete(listener);
      if (sharedListeners.size === 0) {
        stopSharedPolling();
      }
    };
  }, [enabled]);

  return snapshot;
}

export function getLatestSnapshot(): RuntimeSnapshot | null {
  return sharedSnapshot;
}
