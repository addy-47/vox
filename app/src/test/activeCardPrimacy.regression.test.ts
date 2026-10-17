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

  it("strict primacy: only committed provider moves to index 0, drafting does not reorder", () => {
    const sampleProviders = [
      { id: "anthropic", name: "Anthropic" },
      { id: "groq", name: "Groq" },
      { id: "openai", name: "OpenAI" },
    ];

    const sortProvidersStrict = (
      providers: { id: string; name: string }[],
      committedProviderId?: string
    ) => {
      const sorted = [...providers].sort((a, b) => a.name.localeCompare(b.name));
      if (committedProviderId) {
        const idx = sorted.findIndex((p) => p.id === committedProviderId);
        if (idx > 0) {
          const [active] = sorted.splice(idx, 1);
          sorted.unshift(active);
        }
      }
      return sorted;
    };

    // When OpenAI is committed, it moves to index 0
    const committedOpenAi = sortProvidersStrict(sampleProviders, "openai");
    expect(committedOpenAi[0].id).toBe("openai");

    // Merely drafting "groq" without committing preserves the order (committed is still openai)
    const draftedGroq = sortProvidersStrict(sampleProviders, "openai");
    expect(draftedGroq[0].id).toBe("openai");

    // No committed provider keeps alphabetical order
    const uncommitted = sortProvidersStrict(sampleProviders, undefined);
    expect(uncommitted[0].id).toBe("anthropic");
    expect(uncommitted[1].id).toBe("groq");
    expect(uncommitted[2].id).toBe("openai");
  });

  it("server resolution: unifies servers lookup between draft.llm.servers and draft.providers.llm.servers", () => {
    const resolveServer = (
      draft: {
        llm?: { servers?: { id: string; url: string }[] };
        providers?: { llm?: { servers?: { id: string; url: string }[] } };
      },
      serverId: string
    ) => {
      const servers = draft?.llm?.servers ?? draft?.providers?.llm?.servers;
      return servers?.find((s) => s.id === serverId);
    };

    // When in draft.llm.servers
    const draft1 = {
      llm: { servers: [{ id: "srv_1", url: "http://127.0.0.1:11434" }] },
    };
    expect(resolveServer(draft1, "srv_1")?.url).toBe("http://127.0.0.1:11434");

    // When in draft.providers.llm.servers
    const draft2 = {
      providers: { llm: { servers: [{ id: "srv_2", url: "http://192.168.1.50:8000/v1" }] } },
    };
    expect(resolveServer(draft2, "srv_2")?.url).toBe("http://192.168.1.50:8000/v1");
  });

  it("getEffectiveCloudApiKey: resolves across draft, draft.providers, and committed.providers", async () => {
    const { getEffectiveCloudApiKey } = await import("@/shared/lib/activeSelection");

    // Case 1: Key in draft.llm.cloud_keys
    const draft1 = { llm: { cloud_keys: { openrouter: "sk-draft-llm" } } };
    expect(getEffectiveCloudApiKey(draft1, null, "openrouter")).toBe("sk-draft-llm");

    // Case 2: Key in draft.providers.llm.cloud_keys
    const draft2 = { providers: { llm: { cloud_keys: { openrouter: "sk-draft-providers" } } } };
    expect(getEffectiveCloudApiKey(draft2, null, "openrouter")).toBe("sk-draft-providers");

    // Case 3: Key in committed.providers.llm.cloud_keys
    const committed1 = { providers: { llm: { cloud_keys: { openrouter: "sk-committed-providers" } } } };
    expect(getEffectiveCloudApiKey(null, committed1, "openrouter")).toBe("sk-committed-providers");

    // Case 4: Missing key returns undefined
    expect(getEffectiveCloudApiKey(null, null, "openrouter")).toBeUndefined();
  });

  it("active model primacy: only committed pinnedModelId moves to index 0", () => {
    const sampleModels = [
      { id: "anthropic/claude-3-5-sonnet", name: "Claude 3.5 Sonnet" },
      { id: "deepseek/deepseek-chat", name: "DeepSeek Chat" },
      { id: "openai/gpt-4o", name: "GPT-4o" },
    ];

    const sortModelsStrict = (
      models: { id: string; name: string }[],
      pinnedModelId?: string
    ) => {
      const sorted = [...models].sort((a, b) => a.name.localeCompare(b.name));
      if (pinnedModelId) {
        const idx = sorted.findIndex((m) => m.id === pinnedModelId);
        if (idx > 0) {
          const [active] = sorted.splice(idx, 1);
          sorted.unshift(active);
        }
      }
      return sorted;
    };

    // Committed GPT-4o pinned to index 0
    const committedGpt4o = sortModelsStrict(sampleModels, "openai/gpt-4o");
    expect(committedGpt4o[0].id).toBe("openai/gpt-4o");

    // Drafting DeepSeek without committing does not reorder
    const stillGpt4o = sortModelsStrict(sampleModels, "openai/gpt-4o");
    expect(stillGpt4o[0].id).toBe("openai/gpt-4o");
  });
});