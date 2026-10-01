import { invoke } from "@tauri-apps/api/core";

export function showMainWindow(): Promise<void> {
  return invoke("show_main_window");
}

export function hideTrayWindow(): Promise<void> {
  return invoke("hide_tray_window");
}

export function setWindowClickThrough(window: "tray", enabled: boolean): Promise<void> {
  return invoke("set_window_click_through", { window, enabled });
}

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
