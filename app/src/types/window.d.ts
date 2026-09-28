/** Ambient webview globals. Tauri injects `__TAURI_*` at runtime; the Three.js
 * overlay publishes render metrics for the memory profiler. Declared once so
 * no call site needs a window escape hatch. */
interface Window {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
  __TAURI_METADATA__?: { windowLabel?: string };
  __VOX_THREE_METRICS__?: {
    getMetrics?: () => {
      geometries: number;
      textures: number;
      calls: number;
      triangles: number;
      points: number;
      lines: number;
      frame: number;
    };
  };
  webkitAudioContext?: typeof AudioContext;
  __VOX_HIDE_BOOT_LOADER?: () => void;
  requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
}
