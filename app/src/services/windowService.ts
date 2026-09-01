import { invoke } from "@tauri-apps/api/core";

export function showMainWindow(): Promise<void> {
  return invoke("show_main_window");
}

export function hideTrayWindow(): Promise<void> {
  return invoke("hide_tray_window");
}

export function setWindowClickThrough(window: "tray" | "toast", enabled: boolean): Promise<void> {
  return invoke("set_window_click_through", { window, enabled });
}
