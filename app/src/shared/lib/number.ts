/**
 * Parses and bounds a numeric input safely within [min, max].
 * Returns fallback if the value cannot be parsed or is NaN.
 */
export function parseClampedNumber(
  value: unknown,
  min: number,
  max: number,
  fallback: number = min
): number {
  if (typeof value === "number") {
    if (isNaN(value)) return fallback;
    return Math.min(max, Math.max(min, value));
  }
  if (typeof value === "string") {
    const clean = value.replace(/[^0-9.-]/g, "");
    if (!clean) return fallback;
    const parsed = parseFloat(clean);
    if (isNaN(parsed)) return fallback;
    return Math.min(max, Math.max(min, parsed));
  }
  return fallback;
}
