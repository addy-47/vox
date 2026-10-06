import { describe, it, expect } from "vitest";
import {
  deriveNotificationCategoryTokens,
  NOTIFICATION_CATEGORIES,
} from "@/shared/theme/notificationCategoryTokens";

/**
 * Notification category token regression suite (notifications-spec §4.7).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT: accent-derived `--notif-<category>` family — seven harmonic
 * hue-rotated triplets that must never collide with `--accent` itself.
 *
 * Production Entry Seam:
 *   deriveNotificationCategoryTokens(accentTriplet, isLight) — called by
 *   applyNotificationCategoryTokens on every theme/accent write
 *   (shared/theme/index.ts writeThemeToDom), then interpolated by the flip.
 *
 * Direction Check:
 *   Trigger-side derivation: accent triplet in, seven category triplets out.
 *   The test invokes the derivation, not the stylesheet writer or the flip
 *   interpolator (those are downstream sinks with their own invariants).
 *
 * Production Path:
 *   user accent_seed → hexToRgb triplet → deriveNotificationCategoryTokens
 *   → stylesheet rule → flip token interpolation → kicker + action button
 *
 * Observable Exit: the seven "r, g, b" triplets.
 *
 * Phase 2a — Testability: yes (4/4). Pure function, real signature, no
 * constructors needed, output directly observable, no mocked boundary.
 *
 * Phase 2b — False-Green table:
 *   | HUE_OFFSETS gains a 0 entry → a category equals --accent   | must fail |
 *   | isLight ignored → light and dark families identical        | must fail |
 *   | gray-accent guard removed → NaN / achromatic explosion     | must fail |
 *   | malformed triplet throws instead of falling back           | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the never-accent offset guard silently broke, this test would fail
 *   because the "never resolves to accent" assertion would receive the accent
 *   triplet while expecting all seven categories to differ from it.
 */

function parseTriplet(t: string): [number, number, number] {
  const parts = t.split(",").map((s) => Number(s.trim()));
  expect(parts).toHaveLength(3);
  return parts as [number, number, number];
}

function luminance([r, g, b]: [number, number, number]): number {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

describe("notification category tokens (spec §4.7)", () => {
  it("derives seven valid, pairwise-distinct triplets for a saturated accent", () => {
    const tokens = deriveNotificationCategoryTokens("0, 219, 233", false);
    expect(Object.keys(tokens).sort()).toEqual([...NOTIFICATION_CATEGORIES].sort());
    const seen = new Set<string>();
    for (const category of NOTIFICATION_CATEGORIES) {
      const [r, g, b] = parseTriplet(tokens[category]);
      for (const v of [r, g, b]) {
        expect(Number.isInteger(v)).toBe(true);
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(255);
      }
      seen.add(tokens[category]);
    }
    expect(seen.size).toBe(7);
  });

  it("NEGATIVE: no category token may resolve to --accent itself", () => {
    const accent = "0, 219, 233";
    const tokens = deriveNotificationCategoryTokens(accent, false);
    for (const category of NOTIFICATION_CATEGORIES) {
      expect(
        tokens[category],
        `category ${category} collided with --accent`
      ).not.toBe(accent);
    }
  });

  it("light and dark families differ (theme flip has something to interpolate)", () => {
    const dark = deriveNotificationCategoryTokens("0, 219, 233", false);
    const light = deriveNotificationCategoryTokens("0, 219, 233", true);
    let differing = 0;
    for (const category of NOTIFICATION_CATEGORIES) {
      if (dark[category] !== light[category]) differing += 1;
      // Dark theme tokens (l=0.64) must read brighter than light (l=0.44).
      expect(luminance(parseTriplet(dark[category]))).toBeGreaterThan(
        luminance(parseTriplet(light[category]))
      );
    }
    expect(differing).toBe(7);
  });

  it("near-monochrome accents fall back instead of exploding (legend-palette guard)", () => {
    const tokens = deriveNotificationCategoryTokens("128, 128, 128", false);
    const distinct = new Set(Object.values(tokens));
    expect(distinct.size).toBe(7);
    for (const t of Object.values(tokens)) parseTriplet(t);
  });

  it("malformed triplets fall back without throwing", () => {
    expect(() =>
      deriveNotificationCategoryTokens("", false)
    ).not.toThrow();
    const tokens = deriveNotificationCategoryTokens("not-a-color", true);
    expect(Object.keys(tokens)).toHaveLength(7);
  });
});
