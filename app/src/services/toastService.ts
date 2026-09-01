import { invoke } from "@tauri-apps/api/core";
import { type ToastPayload } from "@/services/eventsService";

export type ToastAction = "show" | "hide" | "destroy";

/**
 * Manages native toast overlay window lifecycle states.
 */
export function manageToastWindow(action: ToastAction): Promise<void> {
  return invoke("manage_toast_window", { action });
}

/**
 * Retrieves the last buffered toast payload for late-joining webviews.
 */
export function getLastToast(): Promise<ToastPayload | null> {
  return invoke<ToastPayload | null>("get_last_toast");
}
