import { describe, it, expect, beforeEach, vi } from "vitest";
import { registerOverlay, closeTopmost, getStackSize, getStackIds } from "../overlayStack";

describe("overlayStack (FILO dismissal contract)", () => {
  beforeEach(() => {
    while (closeTopmost()) {
      /* drain */
    }
    vi.restoreAllMocks();
  });

  it("registers overlays in order and closes topmost first (LIFO)", () => {
    const a = vi.fn();
    const b = vi.fn();
    const c = vi.fn();

    const unA = registerOverlay({ onClose: a });
    const unB = registerOverlay({ onClose: b });
    const unC = registerOverlay({ onClose: c });

    expect(getStackSize()).toBe(3);
    expect(getStackIds()).toEqual([1, 2, 3]);

    // Profiler open → popover on top → first Escape closes the popover (C).
    expect(closeTopmost()).toBe(true);
    expect(c).toHaveBeenCalledTimes(1);
    expect(b).not.toHaveBeenCalled();
    expect(a).not.toHaveBeenCalled();

    // Second Escape closes the profiler (B).
    expect(closeTopmost()).toBe(true);
    expect(b).toHaveBeenCalledTimes(1);

    unA();
    unB();
    unC();
  });

  it("drains to empty and reports false when nothing is open", () => {
    const a = vi.fn();
    registerOverlay({ onClose: a });
    expect(closeTopmost()).toBe(true);
    expect(a).toHaveBeenCalledTimes(1);
    expect(getStackSize()).toBe(0);
    expect(closeTopmost()).toBe(false);
  });

  it("unregister removes the overlay from the stack so it is never closed", () => {
    const a = vi.fn();
    const b = vi.fn();
    registerOverlay({ onClose: a });
    const unB = registerOverlay({ onClose: b });

    unB(); // popover dismissed by its own surface (not the stack)
    expect(getStackSize()).toBe(1);

    expect(closeTopmost()).toBe(true);
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).not.toHaveBeenCalled();
  });
});