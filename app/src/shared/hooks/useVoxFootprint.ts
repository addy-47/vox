import type { RuntimeSnapshot } from "@/services/pipelineService";
import { useRuntimeSnapshot } from "@/shared/hooks/useRuntimeSnapshot";

export type { RuntimeSnapshot };

interface VoxFootprint {
  voxCpu: number;
  voxRam: number;
  isReady: boolean;
}

/**
 * useVoxFootprint
 *
 * Reads the shared 30s `get_runtime_snapshot` cache (see useRuntimeSnapshot)
 * and exposes the Vox process's CPU usage and RAM footprint for the
 * bottom-of-screen mini-HUD. No dedicated poller: previously this hook ran
 * its own 2s interval, doubling snapshot IPC traffic alongside the
 * monitoring graphs.
 */
export function useVoxFootprint(): VoxFootprint {
  const snap = useRuntimeSnapshot(true);
  if (!snap) {
    return { voxCpu: 0, voxRam: 0, isReady: false };
  }
  // Coerce to finite numbers: a snapshot that omits these fields (stubbed IPC,
  // older backend) previously produced NaN, and `NaN.toFixed(1)` threw inside
  // the shell render — blanking the whole app.
  const voxCpu = Number.isFinite(snap.vox_cpu_usage) ? snap.vox_cpu_usage : 0;
  const voxRam = Number.isFinite(snap.vox_ram_mb) ? snap.vox_ram_mb : 0;
  return { voxCpu, voxRam, isReady: true };
}
