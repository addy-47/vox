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

    const dark = {
      theme: "dark",
      preset: "vox",
      dark_accent: "#00dbe9",
      light_accent: "#0e7490",
      dark_bg: "#131315",
      light_bg: "#f7f6f2",
    };
    const light = {
      theme: "light",
      preset: "vox",
      dark_accent: "#00dbe9",
      light_accent: "#0e7490",
      dark_bg: "#131315",
      light_bg: "#f7f6f2",
    };

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

  it("swaps active accent and background when flipping themes", () => {
    const appearance = {
      theme: "dark",
      preset: "custom",
      dark_accent: "#8b5cf6",
      light_accent: "#f59e0b",
      dark_bg: "#011627",
      light_bg: "#fafafa",
    };

    applyTheme(appearance, { animate: false });
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("139, 92, 246");
    expect(document.documentElement.style.getPropertyValue("--background")).toBe("1, 22, 39");

    applyTheme({ ...appearance, theme: "light" }, { animate: false });
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("245, 158, 11");
    expect(document.documentElement.style.getPropertyValue("--background")).toBe("250, 250, 250");
  });
});
