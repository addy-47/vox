import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { LlmTuningControls } from "@/shared/components/settings/language/LlmTuningControls";
import { minimalSettings } from "./harness/settingsFixtures";

/**
 * Crash regression: `undefined is not an object (evaluating 'caps.context_window')`.
 *
 * The knobs re-implemented the capabilities lookup and dereferenced the result.
 * Any model absent from `capabilitiesCache` (never probed) threw and took the
 * whole Language card down through the error boundary. The lookup now goes
 * through `resolveCapabilitiesById`, whose contract is "a miss is undefined,
 * never a throw".
 */
describe("LlmTuningControls with an unprobed model", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    const saved = minimalSettings();
    saved.llm.active = { category: "cloud", provider: "groq", model: "never-probed-model" };
    useSettingsStore.setState({
      settings: structuredClone(saved),
      draftSettings: structuredClone(saved),
    });
    useCatalogStore.setState({ capabilitiesCache: {} });
  });

  it("renders the header knobs instead of throwing", () => {
    expect(() => render(<LlmTuningControls />)).not.toThrow();
    expect(screen.getByRole("button", { name: /response length/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /context window/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /creativity/i })).toBeTruthy();
  });

  it("treats an unknown ceiling as server-managed (the control is inert)", () => {
    render(<LlmTuningControls />);
    const context = screen.getByRole("button", { name: /context window/i });
    expect((context as HTMLButtonElement).disabled).toBe(true);
  });

  it("reads the ceiling when the model is cached", () => {
    const saved = minimalSettings();
    saved.llm.active = { category: "cloud", provider: "groq", model: "probed-model" };
    useSettingsStore.setState({
      settings: structuredClone(saved),
      draftSettings: structuredClone(saved),
    });
    useCatalogStore.setState({
      capabilitiesCache: {
        "cloud:probed-model": {
          provider_kind: "cloud",
          model_id: "probed-model",
          context_window: 131072,
        } as never,
      },
    });

    render(<LlmTuningControls />);
    const context = screen.getByRole("button", { name: /context window/i });
    expect((context as HTMLButtonElement).disabled).toBe(false);
  });
});
