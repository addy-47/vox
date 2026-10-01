/**
 * Extracts a human-readable message from an unknown error or rejection reason.
 */
export function getErrorMessage(err: unknown, fallback: string = "Unknown error"): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === "string" && err.trim().length > 0) {
    return err;
  }
  if (err && typeof err === "object" && "message" in err && typeof (err as { message: unknown }).message === "string") {
    return (err as { message: string }).message;
  }
  return err ? String(err) : fallback;
}
