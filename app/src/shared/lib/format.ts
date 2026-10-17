/**
 * Shared byte-size formatter. Single SSOT for file byte display math so
 * decimal precision no longer diverges across surfaces (previously 2dp in
 * the desktop wizard vs 1dp in mobile).
 *
 * Binary-scaled (KiB-style 1024 division) with decimal unit suffixes,
 * matching the convention the wizard model surfaces already established.
 */
export function formatBytes(bytes: number, decimals = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  // Clamp to the largest unit rather than producing "PB"/"EB" outliers.
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  return `${(bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : decimals)} ${units[i]}`;
}
