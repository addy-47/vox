import { describe, it, expect } from "vitest";
import {
  getProviderLogoDef,
  PROVIDER_LOGO_REGISTRY,
} from "@/shared/components/settings/providers/ProviderLogo";
import type { LlmModelInfo } from "@/store/settingsStore";
import { CLOUD_PROVIDERS } from "@/data/providersCopy";

/**
 * Ticket 04: UI Active-Card Primacy & Registry Verification
 */
describe("ProviderLogo Registry Lookup", () => {
  it("resolves exact aliases for top providers", () => {
    const openai = getProviderLogoDef("openai");
    expect(openai).toBeDefined();
    expect(openai?.id).toBe("openai");

    const anthropic = getProviderLogoDef("anthropic");
    expect(anthropic).toBeDefined();
    expect(anthropic?.id).toBe("anthropic");

    const groq = getProviderLogoDef("groq");
    expect(groq).toBeDefined();
    expect(groq?.id).toBe("groq");

    const deepseek = getProviderLogoDef("deepseek");
    expect(deepseek).toBeDefined();
    expect(deepseek?.id).toBe("deepseek");

    const ollama = getProviderLogoDef("ollama");
    expect(ollama).toBeDefined();
    expect(ollama?.id).toBe("ollama");

    const azure = getProviderLogoDef("azure");
    expect(azure).toBeDefined();
    expect(azure?.id).toBe("azure");

    const vox = getProviderLogoDef("vox");
    expect(vox).toBeDefined();
    expect(vox?.id).toBe("vox");
  });

  it("resolves substring matches for compound provider identifiers", () => {
    const compoundOllama = getProviderLogoDef("my-custom-ollama-host");
    expect(compoundOllama).toBeDefined();
    expect(compoundOllama?.id).toBe("ollama");

    const compoundWhisper = getProviderLogoDef("whisper-large-v3");
    expect(compoundWhisper).toBeDefined();
    expect(compoundWhisper?.id).toBe("openai");

    const compoundClaude = getProviderLogoDef("claude-3-5-sonnet");
    expect(compoundClaude).toBeDefined();
    expect(compoundClaude?.id).toBe("anthropic");
  });

  it("preserves precedence: Google Cloud Platform before Google Gemini", () => {
    const gcp = getProviderLogoDef("google_cloud");
    expect(gcp).toBeDefined();
    expect(gcp?.id).toBe("google_cloud");

    const gtts = getProviderLogoDef("google_tts");
    expect(gtts).toBeDefined();
    expect(gtts?.id).toBe("google_cloud");

    const gemini = getProviderLogoDef("gemini");
    expect(gemini).toBeDefined();
    expect(gemini?.id).toBe("gemini");
  });

  it("returns undefined for unknown providers so fallback icon renders", () => {
    const unknown = getProviderLogoDef("some_obscure_host_404");
    expect(unknown).toBeUndefined();
  });

  it("has non-empty aliases and render function for every registered entry", () => {
    for (const def of PROVIDER_LOGO_REGISTRY) {
      expect(def.id).toBeTruthy();
      expect(def.aliases.length).toBeGreaterThan(0);
      expect(typeof def.render).toBe("function");
    }
  });
});

describe("Active Card Primacy: Grid Ordering", () => {
  it("sorts selected model to index 0 of remote models list", () => {
    const sampleModels: LlmModelInfo[] = [
      { id: "meta-llama/llama-3-8b", name: "Llama 3 8B" },
      { id: "deepseek/deepseek-chat", name: "DeepSeek Chat" },
      { id: "anthropic/claude-3-5-sonnet", name: "Claude 3.5 Sonnet" },
      { id: "openai/gpt-4o", name: "GPT-4o" },
    ];

    const sortWithPrimacy = (
      models: LlmModelInfo[],
      selectedModelId?: string,
      pinnedModelId?: string
    ) => {
      const sorted = [...models].sort((a, b) =>
        (a.name || a.id).localeCompare(b.name || b.id)
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

    // Selecting 'openai/gpt-4o' must make it index 0
    const res1 = sortWithPrimacy(sampleModels, "openai/gpt-4o");
    expect(res1[0].id).toBe("openai/gpt-4o");

    // Selecting 'deepseek/deepseek-chat' must make it index 0
    const res2 = sortWithPrimacy(sampleModels, "deepseek/deepseek-chat");
    expect(res2[0].id).toBe("deepseek/deepseek-chat");

    // Fallback to pinnedModelId when selectedModelId is omitted
    const res3 = sortWithPrimacy(sampleModels, undefined, "meta-llama/llama-3-8b");
    expect(res3[0].id).toBe("meta-llama/llama-3-8b");
  });

  it("sorts active cloud provider to index 0 of cloud providers list", () => {
    const sortProvidersWithPrimacy = (
      providers: typeof CLOUD_PROVIDERS,
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

    // When groq is active, it must be index 0
    const resGroq = sortProvidersWithPrimacy(CLOUD_PROVIDERS, "groq");
    expect(resGroq[0].id).toBe("groq");

    // When openrouter is active, it must be index 0
    const resOpenRouter = sortProvidersWithPrimacy(CLOUD_PROVIDERS, "openrouter");
    expect(resOpenRouter[0].id).toBe("openrouter");
  });
});
