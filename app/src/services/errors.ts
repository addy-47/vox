/**
 * Turns any thrown value — Tauri error strings, Error instances, or arbitrary
 * values — into a single user-displayable message.
 */
export function normalizeError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
