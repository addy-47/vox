import { useState, useEffect, useRef } from "react";
import { getRuntimeSnapshot, type RuntimeSnapshot } from "@/services/pipelineService";

// ─── Types ────────────────────────────────────────────────────────────────

export type { RuntimeSnapshot };

interface VoxFootprint {
  voxCpu: number;
  voxRam: number;
  isReady: boolean;
}

const POLL_INTERVAL_MS = 1000;

// ─── Hook ─────────────────────────────────────────────────────────────────────

/**
 * useVoxFootprint
 *
 * Polls `get_runtime_snapshot` at 1Hz and exposes the Vox process's
 * CPU usage and RAM footprint for the bottom-of-screen mini-HUD.
 *
 * Returns raw values (not interpolated) since the mini-HUD is static text,
 * not a chart. Interpolation would make the number feel laggy on a display.
 */
export function useVoxFootprint(): VoxFootprint {
  const [voxCpu, setVoxCpu] = useState<number>(0);
  const [voxRam, setVoxRam] = useState<number>(0);
  const [isReady, setIsReady] = useState<boolean>(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const poll = async () => {
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

    poll(); // immediate first tick
    intervalRef.current = setInterval(poll, POLL_INTERVAL_MS);

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []);

  return { voxCpu, voxRam, isReady };
}
