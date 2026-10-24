import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { CatalogReveal } from "@/shared/components/settings/shared/CatalogReveal";
import { TtsEngineGrid } from "@/shared/components/settings/speech/TtsEngineGrid";
import { SttModelList } from "@/shared/components/settings/speech/SttModelList";
import { VadModelList } from "@/shared/components/settings/speech/VadModelList";
import { SpeechModelCard } from "@/shared/components/settings/speech/SpeechModelCard";
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

const SPEECH_CATALOG = {
  tts: [
    { id: "kokoro", name: "Kokoro", description: "Local", is_cloud: false, is_remote: false, is_built_in: true },
    { id: "edge", name: "Edge", description: "Cloud", is_cloud: true, is_remote: false },
  ],
  stt: [
    { id: "whisper-base", name: "Whisper Base", description: "Base model", is_built_in: false },
    { id: "whisper-large", name: "Whisper Large", description: "Large model", is_built_in: false },
  ],
  vad: [
    { id: "silero-v4", name: "Silero V4", description: "V4 model", is_built_in: true },
    { id: "silero-v5", name: "Silero V5", description: "V5 model", is_built_in: false },
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
    useCatalogStore.setState({ modelCatalog: SPEECH_CATALOG as never });
  });

  afterEach(() => cleanup());

  const gridProps = {
    confirmDeleteId: null,
    setConfirmDeleteId: () => {},
    modelPresence: { kokoro: true, edge: true },
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

  it("promotes the saved (committed) TTS model to the first card in both rows and modal", () => {
    const saved = minimalSettings();
    saved.tts.active.model = "edge";
    useSettingsStore.setState({
      settings: structuredClone(saved),
      draftSettings: structuredClone(saved),
    });

    // In modal (cards variant)
    const { container: modalContainer } = render(
      <TtsEngineGrid {...gridProps} variant="cards" />
    );
    const cards = modalContainer.querySelectorAll("[data-card-id]");
    expect(cards[0].getAttribute("data-card-id")).toBe("edge");
    expect(cards[1].getAttribute("data-card-id")).toBe("kokoro");
    cleanup();

    // In inline catalog (rows variant)
    const { container: rowContainer } = render(<TtsEngineGrid {...gridProps} />);
    const rows = rowContainer.querySelectorAll("[role='button']");
    expect(rows[0].getAttribute("aria-label")).toContain("Edge");
  });

  it("renders a breadcrumb icon for opening voices in the modal and hides delete on hover", () => {
    const { container } = render(
      <TtsEngineGrid
        {...gridProps}
        variant="cards"
        onPickVoice={() => {}}
      />
    );
    const voiceButton = screen.getByLabelText(/open voices: kokoro/i);
    expect(voiceButton).toBeTruthy();
    // ChevronRight has class lucide-chevron-right
    expect(voiceButton.querySelector(".lucide-chevron-right")).toBeTruthy();
    expect(voiceButton.querySelector(".lucide-audio-waveform")).toBeNull();

    // Delete button has opacity-0 group-hover:opacity-100
    const deleteButton = container.querySelector("button[aria-label='Delete weights']");
    expect(deleteButton).toBeTruthy();
    expect(deleteButton?.className).toContain("opacity-0");
    expect(deleteButton?.className).toContain("group-hover:opacity-100");
  });
});

describe("Speech model saved-primacy in STT and VAD catalogs", () => {
  beforeEach(() => {
    const saved = minimalSettings();
    saved.stt.active.model = "whisper-large";
    saved.vad.vad_backend = "silero-v5";
    useSettingsStore.setState({
      settings: structuredClone(saved),
      draftSettings: structuredClone(saved),
    });
    useCatalogStore.setState({ modelCatalog: SPEECH_CATALOG as never });
  });

  afterEach(() => cleanup());

  it("promotes the saved STT model to index 0", () => {
    const { container } = render(
      <SttModelList
        confirmDeleteId={null}
        setConfirmDeleteId={() => {}}
        modelPresence={{ "whisper-base": true, "whisper-large": true }}
        downloadStatuses={{}}
        startDownload={() => {}}
        deleteModel={() => {}}
        isGroupRequired={() => false}
      />
    );
    const rows = container.querySelectorAll("[role='button']");
    expect(rows[0].getAttribute("aria-label")).toBe("Whisper Large");
    expect(rows[1].getAttribute("aria-label")).toBe("Whisper Base");
  });

  it("promotes the saved VAD model to index 0", () => {
    const { container } = render(
      <VadModelList
        confirmDeleteId={null}
        setConfirmDeleteId={() => {}}
        modelPresence={{ "silero-v4": true, "silero-v5": true }}
        downloadStatuses={{}}
        startDownload={() => {}}
        deleteModel={() => {}}
      />
    );
    const rows = container.querySelectorAll("[role='button']");
    expect(rows[0].getAttribute("aria-label")).toBe("Silero V5");
    expect(rows[1].getAttribute("aria-label")).toBe("Silero V4");
  });
});

describe("SpeechModelCard hover-only controls", () => {
  afterEach(() => cleanup());

  it("only shows info tooltip and delete icon on hover", () => {
    const { container } = render(
      <SpeechModelCard
        id="test-model"
        name="Test Model"
        description="A test model description"
        parameters="1.5B"
        isDownloaded={true}
        isActive={false}
        confirmDeleteId={null}
        setConfirmDeleteId={() => {}}
        startDownload={() => {}}
        deleteModel={() => {}}
        onSelect={() => {}}
      />
    );

    const infoTrigger = container.querySelector("span[aria-label='Test Model details']");
    expect(infoTrigger).toBeTruthy();
    expect(infoTrigger?.className).toContain("opacity-0");
    expect(infoTrigger?.className).toContain("group-hover:opacity-100");

    const deleteButton = container.querySelector("button[aria-label='Delete weights']");
    expect(deleteButton).toBeTruthy();
    expect(deleteButton?.className).toContain("opacity-0");
    expect(deleteButton?.className).toContain("group-hover:opacity-100");
  });
});
