import { invoke } from "@tauri-apps/api/core";

export function showMainWindow(): Promise<void> {
  return invoke("show_main_window");
}

export function hideTrayWindow(): Promise<void> {
  return invoke("hide_tray_window");
}

export function syncHudVisibility(visible: boolean): Promise<void> {
  return invoke("sync_hud_visibility", { visible });
}

export function setHudIgnoreCursor(ignore: boolean): Promise<void> {
  return invoke("set_hud_ignore_cursor", { ignore });
}
