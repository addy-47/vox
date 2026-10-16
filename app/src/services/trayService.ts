import { invoke } from "./transport";

export function hideTrayWindow(): Promise<void> {
  return invoke("hide_tray_window");
}

export function setWindowClickThrough(window: "tray", enabled: boolean): Promise<void> {
  return invoke("set_window_click_through", { window, enabled });
}