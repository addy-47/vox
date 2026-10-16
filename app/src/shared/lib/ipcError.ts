/**
 * Extracts a human-readable message from any IPC failure shape. Tauri
 * rejects with a structured `{ error_type, message }` object — not an
 * `Error` — so `String(err)` renders as `[object Object]`. Every catch
 * block on an IPC boundary must go through here.
 */
export function ipcErrorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message || String(err);
  if (typeof err === "object" && err !== null) {
    const record = err as Record<string, unknown>;
    if (typeof record.message === "string" && record.message.length > 0) {
      return record.message;
    }
    try {
      return JSON.stringify(err);
    } catch {
      return "Unknown error";
    }
  }
  return String(err);
}
