import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { setTransport, type Transport, type UnlistenFn } from "@/services/transport";
import {
  ModalProviderFooter,
  SettingsCommitControls,
} from "@/shared/components/settings/SettingsCommitControls";
import { minimalSettings } from "./harness/settingsFixtures";

/**
 * Modal commit controls regression suite.
 *
 * Two defects are covered:
 *  1. The provider modal footer rendered TWO banners for ONE cause: the
 *     missing-credential banner and a "Connection failed" banner whose detail
 *     was the very same "API key required" string.
 *  2. The modal header commit control (Apply & Restart / Changes Saved) had to
 *     be present in every settings modal and to collapse to an icon-only
 *     "Restart" button on small and mobile widths.
 *
 * Production seams exercised: the real settings store (draft + domain flags
 * derived through the real contract reload policies), the real catalog store
 * error channel, and the real components. Only the transport is swapped — the
 * swappable boundary the production code documents.
 */

const CONTRACT = {
  supported_keys: { llm: ["active", "last_selected", "servers", "cloud_keys"] },
  reload_policies: { llm: { active: "restart", last_selected: "restart", servers: "restart", cloud_keys: "hot" } },
  secret_keys: [],
  llm_cloud_providers: [],
  tts_providers: [],
  defaults: { temperature: 0.7, max_output_tokens: 512, max_tokens: 512, speed: 1 },
};

function contractTransport(): Transport {
  return {
    invoke<T>(cmd: string): Promise<T> {
      if (cmd === "get_settings_contract") {
        return Promise.resolve(structuredClone(CONTRACT) as unknown as T);
      }
      return Promise.resolve(null as unknown as T);
    },
    listen(): Promise<UnlistenFn> {
      return Promise.resolve(() => {});
    },
    convertFileSrc(filePath: string): string {
      return filePath;
    },
  } as Transport;
}

describe("modal commit controls", () => {
  beforeEach(async () => {
    setTransport(contractTransport());
    await useSettingsStore.getState().loadContract();
    useCatalogStore.setState({ remoteModelsError: null });
    useSettingsStore.setState({
      settings: null,
      draftSettings: null,
      failedSaveDomains: {},
      restartKeys: [],
      autoSavedDomain: null,
      domainFlags: {},
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows ONE banner when the cloud provider has no key", () => {
    const saved = minimalSettings();
    const draft = structuredClone(saved);
    draft.llm.active = { category: "cloud", provider: "openai", model: "gpt-x" };
    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useCatalogStore.setState({
      remoteModelsError: "API key required. Configure your key in Providers.",
    });

    render(<ModalProviderFooter category="LLM" />);

    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(1);
    expect(alerts[0]?.textContent).toMatch(/api key required/i);
    expect(screen.queryByText(/connection failed/i)).toBeNull();
  });

  it("still reports a genuine connection failure as a connection failure", () => {
    const saved = minimalSettings();
    useSettingsStore.setState({ settings: saved, draftSettings: structuredClone(saved) });
    useCatalogStore.setState({ remoteModelsError: "ECONNREFUSED 127.0.0.1:11434" });

    render(<ModalProviderFooter category="LLM" />);

    expect(screen.getByText(/connection failed/i)).toBeTruthy();
    expect(screen.queryByText(/api key required for cloud provider/i)).toBeNull();
  });

  it("exposes Apply & Restart in the header once the domain needs a restart", async () => {
    const saved = minimalSettings();
    const draft = structuredClone(saved);
    draft.llm.active = { category: "server", provider: "srv-1", model: "llama" };
    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();

    render(<SettingsCommitControls domainId="language" />);

    const button = screen.getByRole("button", { name: /apply & restart/i });
    expect(button).toBeTruthy();
  });

  it("collapses the header button to an icon-only Restart on mobile widths", async () => {
    const saved = minimalSettings();
    const draft = structuredClone(saved);
    draft.llm.active = { category: "server", provider: "srv-1", model: "llama" };
    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();

    const originalWidth = window.innerWidth;
    window.innerWidth = 400;
    try {
      render(<SettingsCommitControls domainId="language" />);
      const button = screen.getByRole("button", { name: /^restart$/i });
      expect(button).toBeTruthy();
      expect(button.textContent?.trim()).not.toMatch(/apply/i);
    } finally {
      window.innerWidth = originalWidth;
    }
  });
});
