import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { setTransport, type Transport, type UnlistenFn } from "@/services/transport";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { resolveProviderModel } from "@/shared/lib/activeSelection";
import { useProviderSelection } from "@/shared/hooks/useProviderSelection";
import { isLlmSelectionMissingModel } from "@/shared/components/settings/SettingsCommitControls";
import { minimalSettings } from "./harness/settingsFixtures";
import type { CloudProvider } from "@/data/providersCopy";

/**
 * Provider-switch root cause: "Apply & Restart" that could not clear.
 *
 * Defect: every provider switch copied the *current* model id into the new
 * selection. A model id belongs to the provider that serves it, so going
 * saved-provider → other → On-Device → back to the saved provider left
 * `{cloud, savedId, <embedded model>}` against committed
 * `{cloud, savedId, <cloud model>}`. `checkDomainDirty` stayed true, so the
 * footer offered a restart for a configuration identical to the saved one.
 *
 * Production seam: the real settings store (draft + derived domain flags),
 * the real `useProviderSelection` handlers, the real `resolveProviderModel`.
 * Only the transport is swapped.
 */

const CONTRACT = {
  supported_keys: { llm: ["active", "last_selected", "servers", "cloud_keys"] },
  reload_policies: {
    llm: { active: "restart", last_selected: "restart", servers: "restart", cloud_keys: "hot" },
  },
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

const provider = (id: string): CloudProvider => ({
  id,
  name: id,
  url: "",
  keyPlaceholder: "",
});

const SAVED_CLOUD = { category: "cloud" as const, provider: "openai", model: "gpt-x" };

describe("resolveProviderModel", () => {
  it("restores the committed model when the target is the committed provider", () => {
    expect(
      resolveProviderModel({ category: "cloud", provider: "openai" }, SAVED_CLOUD)
    ).toBe("gpt-x");
  });

  it("never carries a foreign model id into another provider", () => {
    expect(
      resolveProviderModel({ category: "cloud", provider: "groq" }, SAVED_CLOUD)
    ).toBe("");
  });

  it("uses the caller fallback for a non-committed target", () => {
    expect(
      resolveProviderModel({ category: "embedded", provider: null }, SAVED_CLOUD, "qwen")
    ).toBe("qwen");
  });

  it("restores the committed embedded model", () => {
    const committed = { category: "embedded" as const, provider: null, model: "qwen" };
    expect(
      resolveProviderModel({ category: "embedded", provider: null }, committed, "other")
    ).toBe("qwen");
  });

  it("treats a missing provider as null on both sides", () => {
    expect(
      resolveProviderModel(
        { category: "embedded", provider: null },
        { category: "embedded", provider: null, model: "qwen" },
        ""
      )
    ).toBe("qwen");
  });
});

describe("language domain dirty state across provider switches", () => {
  beforeEach(async () => {
    setTransport(contractTransport());
    await useSettingsStore.getState().loadContract();
    useCatalogStore.setState({ remoteModelsError: null });

    const saved = minimalSettings();
    saved.llm.active = { ...SAVED_CLOUD };
    saved.llm.last_selected = { embedded: "qwen" };
    useSettingsStore.setState({
      settings: structuredClone(saved),
      draftSettings: structuredClone(saved),
      failedSaveDomains: {},
      restartKeys: [],
      autoSavedDomain: null,
      domainFlags: {},
    });
    useSettingsStore.getState().recomputeDomainFlags();
  });

  afterEach(() => {
    cleanup();
  });

  const isDirty = () => useSettingsStore.getState().isDomainDirty("language");

  it("is clean before any edit", () => {
    expect(isDirty()).toBe(false);
  });

  it("clears after returning to the saved provider", () => {
    const { result } = renderHook(() => useProviderSelection({ category: "LLM" }));

    act(() => result.current.onSelectProvider(provider("groq")));
    expect(isDirty()).toBe(true);

    act(() => result.current.onSelectProvider(provider("openai")));
    expect(useSettingsStore.getState().draftSettings?.llm.active).toEqual(SAVED_CLOUD);
    expect(isDirty()).toBe(false);
  });

  it("clears after a detour through On-Device", () => {
    const { result } = renderHook(() => useProviderSelection({ category: "LLM" }));

    act(() => result.current.onSelectProvider(provider("groq")));
    act(() => result.current.onSelectLocal());
    expect(useSettingsStore.getState().draftSettings?.llm.active).toEqual({
      category: "embedded",
      provider: null,
      model: "qwen",
    });

    act(() => result.current.onSelectProvider(provider("openai")));
    expect(useSettingsStore.getState().draftSettings?.llm.active).toEqual(SAVED_CLOUD);
    expect(isDirty()).toBe(false);
  });

  it("marks a cloud selection with no model as non-committable", () => {
    const { result } = renderHook(() => useProviderSelection({ category: "LLM" }));
    act(() => result.current.onSelectProvider(provider("groq")));

    const active = useSettingsStore.getState().draftSettings?.llm.active;
    expect(active?.model).toBe("");
    expect(isLlmSelectionMissingModel(active)).toBe(true);
  });
});
