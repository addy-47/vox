/**
 * Label of the webview this code is running in ("main" | "tray" | "wizard").
 *
 * Used to keep the heavyweight app providers (pipeline listeners, spatial
 * navigation, profiler) out of the setup wizard webview. Without this the whole
 * app booted twice on a first run — once in `main` and once in `wizard` — which
 * on an 8GB CPU-only box doubled the pipeline listeners and startup IPC.
 *
 * Returns null when Tauri is unavailable (browser preview / unit tests).
 */
export async function getCurrentWindowLabel(): Promise<string | null> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().label;
  } catch {
    return null;
  }
}

/**
 * Synchronous check for wizard role to eliminate initial mount flash.
 * Checks path, query parameters, and synchronous Tauri metadata if present.
 */
export function isWizardWindowSync(): boolean {
  if (typeof window === "undefined") return false;
  if (window.location.pathname.startsWith("/wizard") || window.location.search.includes("mode=wizard")) {
    return true;
  }
  const label = (window as unknown as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } })
    .__TAURI_INTERNALS__?.metadata?.currentWindow?.label;
  if (label) return label === "wizard";
  return false;
}
