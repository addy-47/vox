import { invoke } from "@tauri-apps/api/core";
import { type ToastPayload } from "@/services/eventsService";

export type ToastAction = "show" | "hide" | "destroy";

export function showMainWindow(): Promise<void> {
  return invoke("show_main_window");
}

export function hideTrayWindow(): Promise<void> {
  return invoke("hide_tray_window");
}

export function setWindowClickThrough(window: "tray" | "toast", enabled: boolean): Promise<void> {
  return invoke("set_window_click_through", { window, enabled });
}

export function manageToastWindow(action: ToastAction): Promise<void> {
  return invoke("manage_toast_window", { action });
}

export function getLastToast(): Promise<ToastPayload | null> {
  return invoke<ToastPayload | null>("get_last_toast");
}
