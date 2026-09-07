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
  return { voxCpu: snap.vox_cpu_usage, voxRam: snap.vox_ram_mb, isReady: true };
}
