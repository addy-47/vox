/**
 * Safely copy text to clipboard across platforms (WebKitGTK, Linux, Tauri, browser).
 * Prevents unhandled promise rejections if permissions are denied or clipboard API fails,
 * and falls back to standard document.execCommand('copy').
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  if (!text) return false;

  // 1. Try modern async Clipboard API if available
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      // Permission denied or context not focused — fall through to execCommand
    }
  }

  // 2. Fallback to execCommand('copy') via temporary textarea
  try {
    if (typeof document !== "undefined") {
      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.style.position = "fixed";
      textarea.style.left = "-9999px";
      textarea.style.top = "-9999px";
      textarea.style.opacity = "0";
      textarea.setAttribute("readonly", "");
      document.body.appendChild(textarea);
      textarea.focus();
      textarea.select();
      const success = document.execCommand("copy");
      document.body.removeChild(textarea);
      return success;
    }
  } catch (err) {
    console.warn("[clipboard] Copy fallback failed:", err);
  }

  return false;
}
