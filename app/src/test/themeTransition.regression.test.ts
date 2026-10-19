import { afterEach, describe, expect, it, vi } from "vitest";
import { applyTheme, resetThemeStateForTests } from "@/shared/theme";

describe("theme transition token snapshot", () => {
  afterEach(() => {
    resetThemeStateForTests();
    vi.restoreAllMocks();
  });

  it("reuses the last interpolated root tokens on a later flip", () => {
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const getComputedStyle = vi.spyOn(window, "getComputedStyle");

    const dark = { theme: "dark", accent_seed: "#00dbe9" };
    const light = { theme: "light", accent_seed: "#00dbe9" };

    applyTheme(dark, { animate: false });
    const initialStyleReads = getComputedStyle.mock.calls.length;

    applyTheme(light);
    expect(getComputedStyle.mock.calls.length).toBeGreaterThan(initialStyleReads);
    const firstFlipStyleReads = getComputedStyle.mock.calls.length;

    frames.shift()?.(performance.now() + 200);
    applyTheme(dark);

    expect(getComputedStyle.mock.calls.length).toBe(firstFlipStyleReads);
    frames.shift()?.(performance.now() + 200);
  });
});
