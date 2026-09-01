import { useState, useEffect, useRef } from "react";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";


export type { RuntimeSnapshot };

interface VoxFootprint {
  voxCpu: number;
  voxRam: number;
  isReady: boolean;
}

const POLL_INTERVAL_MS = 2000;


/**
 * useVoxFootprint
 *
 * Polls `get_runtime_snapshot` at 2s (gated by document visibility) and exposes
 * the Vox process's CPU usage and RAM footprint for the bottom-of-screen mini-HUD.
 */
export function useVoxFootprint(): VoxFootprint {
  const [voxCpu, setVoxCpu] = useState<number>(0);
  const [voxRam, setVoxRam] = useState<number>(0);
  const [isReady, setIsReady] = useState<boolean>(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const poll = async () => {
      if (document.hidden) return;
      try {
        const snap = await getRuntimeSnapshot();
        if (snap) {
          setVoxCpu(snap.vox_cpu_usage);
          setVoxRam(snap.vox_ram_mb);
          setIsReady(true);
        }
      } catch {
        // Silent — monitoring is best-effort, never crash the layout
      }
    };

    const startPolling = () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
      poll();
      intervalRef.current = setInterval(poll, POLL_INTERVAL_MS);
    };

    const stopPolling = () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };

    const onVisibilityChange = () => {
      if (document.hidden) {
        stopPolling();
      } else {
        startPolling();
      }
    };

    if (!document.hidden) {
      startPolling();
    }

    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      stopPolling();
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, []);

  return { voxCpu, voxRam, isReady };
}
