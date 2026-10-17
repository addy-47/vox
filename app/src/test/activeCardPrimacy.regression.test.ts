import { describe, it, expect } from "vitest";
import {
  getProviderLogoDef,
  PROVIDER_LOGO_REGISTRY,
} from "@/shared/components/settings/providers/ProviderLogo";
import type { LlmModelInfo } from "@/store/settingsStore";

/**
 * Ticket 04 (updated after ZBC purge): registry empty, logos data-driven.
 */
describe("ProviderLogo Registry Lookup", () => {
  it("returns undefined for every provider id", () => {
    expect(getProviderLogoDef("openai")).toBeUndefined();
    expect(getProviderLogoDef("anthropic")).toBeUndefined();
    expect(getProviderLogoDef("groq")).toBeUndefined();
    expect(getProviderLogoDef("ollama")).toBeUndefined();
    expect(getProviderLogoDef("some_obscure_host_404")).toBeUndefined();
  });

  it("has an empty registry after ZBC", () => {
    expect(PROVIDER_LOGO_REGISTRY).toEqual([]);
  });
});

describe("Active Card Primacy: Grid Ordering", () => {
  it("sorts selected model to index 0 of remote models list", () => {
    const sampleModels: LlmModelInfo[] = [
      { id: "meta-llama/llama-3-8b", name: "Llama 3 8B", size_bytes: 0, quantization: "q4", family: "llama", provider_kind: "cloud" },
      { id: "deepseek/deepseek-chat", name: "DeepSeek Chat", size_bytes: 0, quantization: "q4", family: "deepseek", provider_kind: "cloud" },
      { id: "anthropic/claude-3-5-sonnet", name: "Claude 3.5 Sonnet", size_bytes: 0, quantization: "q4", family: "claude", provider_kind: "cloud" },
      { id: "openai/gpt-4o", name: "GPT-4o", size_bytes: 0, quantization: "q4", family: "gpt", provider_kind: "cloud" },
    ];

    const sortWithPrimacy = (
      models: LlmModelInfo[],
      selectedModelId?: string,
      pinnedModelId?: string
    ) => {
      const sorted = [...models].sort((a, b) =>
        (a.name || a.id).localeCompare(b.name || a.id)
      );
      const activeModelId = selectedModelId || pinnedModelId;
      if (activeModelId) {
        const idx = sorted.findIndex((m) => m.id === activeModelId);
        if (idx > 0) {
          const [selected] = sorted.splice(idx, 1);
          sorted.unshift(selected);
        }
      }
      return sorted;
    };

    const res1 = sortWithPrimacy(sampleModels, "openai/gpt-4o");
    expect(res1[0].id).toBe("openai/gpt-4o");

    const res2 = sortWithPrimacy(sampleModels, "deepseek/deepseek-chat");
    expect(res2[0].id).toBe("deepseek/deepseek-chat");

    const res3 = sortWithPrimacy(sampleModels, undefined, "meta-llama/llama-3-8b");
    expect(res3[0].id).toBe("meta-llama/llama-3-8b");
  });

  it("sorts active cloud provider to index 0 of cloud providers list", () => {
    const sampleProviders: { id: string; name: string }[] = [
      { id: "groq", name: "Groq" },
      { id: "openrouter", name: "OpenRouter" },
    ];
    const sortProvidersWithPrimacy = (
      providers: { id: string; name: string }[],
      activeCloudId: string | null
    ) => {
      const sorted = [...providers].sort((a, b) => a.name.localeCompare(b.name));
      if (activeCloudId) {
        const idx = sorted.findIndex((p) => p.id === activeCloudId);
        if (idx > 0) {
          const [active] = sorted.splice(idx, 1);
          sorted.unshift(active);
        }
      }
      return sorted;
    };

    const resGroq = sortProvidersWithPrimacy(sampleProviders, "groq");
    expect(resGroq[0].id).toBe("groq");

    const resOpenRouter = sortProvidersWithPrimacy(sampleProviders, "openrouter");
    expect(resOpenRouter[0].id).toBe("openrouter");
  });
});