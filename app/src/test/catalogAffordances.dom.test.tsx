import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { CatalogReveal } from "@/shared/components/settings/shared/CatalogReveal";
import { TtsEngineGrid } from "@/shared/components/settings/speech/TtsEngineGrid";
import { minimalSettings } from "./harness/settingsFixtures";

/**
 * Catalog affordance regressions.
 *
 * 1. The reveal label promised "Show all" but opened the modal — the list is a
 *    native scroller that already holds every row, so nothing was truncated.
 *    It now reads "Expand" and carries the maximize glyph.
 * 2. The TTS engine tiles are modal-only. The inline face keeps rows; rendering
 *    tiles in a 260px card is what the variant prop prevents.
 */

const TTS_CATALOG = {
  tts: [
    { id: "kokoro", name: "Kokoro", description: "Local", is_cloud: false, is_remote: false, is_built_in: true },
    { id: "edge", name: "Edge", description: "Cloud", is_cloud: true, is_remote: false },
  ],
};

/** jsdom implements neither observer, and the reveal needs one of them. */
class NoopIntersectionObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}
class NoopResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  globalThis.IntersectionObserver = NoopIntersectionObserver as never;
  globalThis.ResizeObserver = NoopResizeObserver as never;
});

describe("CatalogReveal affordance", () => {
  afterEach(() => cleanup());

  it("offers one Expand affordance with the maximize glyph", () => {
    render(
      <CatalogReveal>
        <div>row</div>
      </CatalogReveal>
    );

    const trigger = screen.getByRole("button", { name: "Expand" });
    expect(trigger.textContent).toBe("Expand");
    expect(trigger.querySelector("svg")).toBeTruthy();
    expect(screen.queryByText(/show all/i)).toBeNull();
  });
});

describe("TtsEngineGrid variants", () => {
  beforeEach(() => {
    const saved = minimalSettings();
    useSettingsStore.setState({
      settings: structuredClone(saved),
      draftSettings: structuredClone(saved),
    });
    useCatalogStore.setState({ modelCatalog: TTS_CATALOG as never });
  });

  afterEach(() => cleanup());

  const gridProps = {
    confirmDeleteId: null,
    setConfirmDeleteId: () => {},
    modelPresence: { kokoro: true },
    downloadStatuses: {},
    startDownload: () => {},
    deleteModel: () => {},
  };

  it("renders rows by default (the inline card face)", () => {
    const { container } = render(<TtsEngineGrid {...gridProps} />);
    expect(container.querySelector("[data-card-id]")).toBeNull();
  });

  it("renders tiles in the cards variant (the modal)", () => {
    const { container } = render(<TtsEngineGrid {...gridProps} variant="cards" />);
    expect(container.querySelectorAll("[data-card-id]")).toHaveLength(2);
    expect(screen.getByText("Kokoro")).toBeTruthy();
  });
});
