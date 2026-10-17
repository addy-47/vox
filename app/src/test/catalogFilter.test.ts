import { describe, expect, it } from "vitest";
import {
  filterProviders,
  filterModels,
  filterVoices,
  isVoiceEnglish,
  type ProviderFilterItem,
} from "../shared/lib/catalogFilter";
import type { LlmModelInfo, ModelCapabilities } from "../store/settingsStore";
import type { UnifiedVoiceDto } from "../services/voicesService";

describe("catalogFilter - filterProviders", () => {
  const sampleProviders: ProviderFilterItem[] = [
    { id: "openai", name: "OpenAI", tagline: "Industry standard GPT models", url: "https://api.openai.com", category: "cloud" },
    { id: "anthropic", name: "Anthropic", tagline: "Claude reasoning engine", url: "https://api.anthropic.com", category: "cloud" },
    { id: "local-whisper", name: "Whisper Local", tagline: "On-device STT", url: null, category: "local", isFree: true },
    { id: "custom-ollama", name: "Ollama Self-Hosted", tagline: "Local inference", url: "http://localhost:11434", category: "server" },
    { id: "openrouter", name: "OpenRouter", tagline: "Unified aggregator", url: "https://openrouter.ai", category: "cloud" },
  ];

  const keys: Record<string, string> = {
    openai: "sk-proj-12345",
  };
  const getProviderKey = (id: string) => keys[id];

  it("returns all providers when filter is 'all' and search is empty", () => {
    const result = filterProviders(sampleProviders, "all", "");
    expect(result).toHaveLength(5);
  });

  it("filters by text search across name, id, tagline, and url", () => {
    expect(filterProviders(sampleProviders, "all", "claude")).toHaveLength(1);
    expect(filterProviders(sampleProviders, "all", "claude")[0].id).toBe("anthropic");

    expect(filterProviders(sampleProviders, "all", "openai.com")).toHaveLength(1);
    expect(filterProviders(sampleProviders, "all", "openai.com")[0].id).toBe("openai");

    expect(filterProviders(sampleProviders, "all", "localhost")).toHaveLength(1);
    expect(filterProviders(sampleProviders, "all", "localhost")[0].id).toBe("custom-ollama");
  });

  it("filters by 'local' category", () => {
    const result = filterProviders(sampleProviders, "local", "");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("local-whisper");
  });

  it("filters by 'cloud' category", () => {
    const result = filterProviders(sampleProviders, "cloud", "");
    expect(result.map((p) => p.id)).toEqual(["openai", "anthropic", "openrouter"]);
  });

  it("filters by 'configured' status considering keys, free tier, and local/server", () => {
    const result = filterProviders(sampleProviders, "configured", "", { getProviderKey });
    // openai has key, local-whisper is free/local, custom-ollama is server
    expect(result.map((p) => p.id)).toEqual(["openai", "local-whisper", "custom-ollama"]);
  });

  it("filters by 'unconfigured' status identifying missing keys", () => {
    const result = filterProviders(sampleProviders, "unconfigured", "", { getProviderKey });
    expect(result.map((p) => p.id)).toEqual(["anthropic", "openrouter"]);
  });
});

describe("catalogFilter - filterModels", () => {
  const sampleModels: LlmModelInfo[] = [
    {
      id: "gpt-4o",
      name: "GPT-4o",
      family: "gpt",
      quantization: null,
      size_bytes: null,
      provider_kind: "cloud",
      capabilities: { tps: 65, supports_tools: "supported" } as unknown as ModelCapabilities,
    },
    {
      id: "claude-3-5-sonnet",
      name: "Claude 3.5 Sonnet",
      family: "claude",
      quantization: null,
      size_bytes: null,
      provider_kind: "cloud",
      capabilities: { tps: 45, supports_tools: "supported" } as unknown as ModelCapabilities,
    },
    {
      id: "llama-3-8b-q4",
      name: "Llama 3 8B Q4",
      family: "llama",
      quantization: "Q4_K_M",
      size_bytes: 4900000000,
      provider_kind: "server",
      capabilities: { tps: 22, supports_tools: "unsupported" } as unknown as ModelCapabilities,
    },
    {
      id: "mistral-7b",
      name: "Mistral 7B",
      family: "mistral",
      quantization: "Q8_0",
      size_bytes: 7500000000,
      provider_kind: "server",
      capabilities: { tps: 0, supports_tools: "unknown" } as unknown as ModelCapabilities,
    },
  ];

  it("filters by search query across id, name, family, and quantization", () => {
    expect(filterModels(sampleModels, "all", "sonnet")).toHaveLength(1);
    expect(filterModels(sampleModels, "all", "sonnet")[0].id).toBe("claude-3-5-sonnet");

    expect(filterModels(sampleModels, "all", "Q4_K_M")).toHaveLength(1);
    expect(filterModels(sampleModels, "all", "Q4_K_M")[0].id).toBe("llama-3-8b-q4");

    expect(filterModels(sampleModels, "all", "mistral")).toHaveLength(1);
    expect(filterModels(sampleModels, "all", "mistral")[0].id).toBe("mistral-7b");
  });

  it("filters by favorites category", () => {
    const favoriteIds = new Set(["gpt-4o", "mistral-7b"]);
    const result = filterModels(sampleModels, { category: "favorites", favoriteIds });
    expect(result.map((m) => m.id)).toEqual(["gpt-4o", "mistral-7b"]);
  });

  it("filters by benchmarked category (tps > 0)", () => {
    const result = filterModels(sampleModels, "benchmarked");
    expect(result.map((m) => m.id)).toEqual(["gpt-4o", "claude-3-5-sonnet", "llama-3-8b-q4"]);
  });

  it("filters by tools support category", () => {
    const result = filterModels(sampleModels, "tools");
    expect(result.map((m) => m.id)).toEqual(["gpt-4o", "claude-3-5-sonnet"]);
  });

  it("filters by fast category (tps >= 30)", () => {
    const result = filterModels(sampleModels, "fast");
    expect(result.map((m) => m.id)).toEqual(["gpt-4o", "claude-3-5-sonnet"]);
  });

  it("supports custom resolveProbed capabilities resolver", () => {
    const customProbe = (m: LlmModelInfo) => {
      if (m.id === "mistral-7b") return { tps: 90, supports_tools: "supported" };
      return m.capabilities || undefined;
    };
    const result = filterModels(sampleModels, { category: "fast", resolveProbed: customProbe });
    expect(result.map((m) => m.id)).toEqual(["gpt-4o", "claude-3-5-sonnet", "mistral-7b"]);
  });
});

describe("catalogFilter - filterVoices", () => {
  const sampleVoices: UnifiedVoiceDto[] = [
    {
      id: "af_bella",
      provider_id: "kokoro",
      name: "Bella",
      description: "Warm and friendly American voice",
      gender: "female",
      locale: "en-US",
      accent: "American",
      tags: ["warm", "expressive"],
      sample_text: "Hello there!",
      source_kind: "bundled",
      is_favorite: true,
      last_used_at: 1700000000,
      has_preview: true,
    },
    {
      id: "am_adam",
      provider_id: "kokoro",
      name: "Adam",
      description: "Deep narrative male voice",
      gender: "male",
      locale: "en-US",
      accent: "American",
      tags: ["deep", "narration"],
      sample_text: "Deep narrative voice.",
      source_kind: "bundled",
      is_favorite: false,
      last_used_at: null,
      has_preview: true,
    },
    {
      id: "fr_julie",
      provider_id: "piper",
      name: "Julie",
      description: "French natural female voice",
      gender: "female",
      locale: "fr-FR",
      accent: "French",
      tags: ["conversational"],
      sample_text: "Bonjour tout le monde.",
      source_kind: "bundled",
      is_favorite: false,
      last_used_at: null,
      has_preview: true,
    },
    {
      id: "custom_clone_1",
      provider_id: "chatterbox",
      name: "My Custom Voice",
      description: "Cloned user voice",
      gender: "neutral",
      locale: "en-GB",
      accent: "British",
      tags: ["clone", "custom"],
      sample_text: "Testing custom clone.",
      source_kind: "custom",
      is_favorite: true,
      last_used_at: 1710000000,
      has_preview: false,
    },
  ];

  it("correctly identifies English locales via isVoiceEnglish", () => {
    expect(isVoiceEnglish({ locale: "en-US" })).toBe(true);
    expect(isVoiceEnglish({ locale: "en-GB" })).toBe(true);
    expect(isVoiceEnglish({ locale: "fr-FR" })).toBe(false);
    expect(isVoiceEnglish({ locale: "de-DE" })).toBe(false);
  });

  it("filters by providerId scope", () => {
    const result = filterVoices(sampleVoices, { providerId: "kokoro" });
    expect(result.map((v) => v.id)).toEqual(["af_bella", "am_adam"]);
  });

  it("filters by recent scope", () => {
    const result = filterVoices(sampleVoices, { scope: "recent" });
    expect(result.map((v) => v.id)).toEqual(["af_bella", "custom_clone_1"]);
  });

  it("filters by favorites scope", () => {
    const result = filterVoices(sampleVoices, { scope: "favorites" });
    expect(result.map((v) => v.id)).toEqual(["af_bella", "custom_clone_1"]);
  });

  it("filters by custom cloned scope", () => {
    const result = filterVoices(sampleVoices, { scope: "custom" });
    expect(result.map((v) => v.id)).toEqual(["custom_clone_1"]);
  });

  it("filters by gender", () => {
    expect(filterVoices(sampleVoices, { gender: "female" }).map((v) => v.id)).toEqual(["af_bella", "fr_julie"]);
    expect(filterVoices(sampleVoices, { gender: "male" }).map((v) => v.id)).toEqual(["am_adam"]);
    expect(filterVoices(sampleVoices, { gender: "neutral" }).map((v) => v.id)).toEqual(["custom_clone_1"]);
  });

  it("filters by accent (english vs multilingual)", () => {
    expect(filterVoices(sampleVoices, { accent: "english" }).map((v) => v.id)).toEqual(["af_bella", "am_adam", "custom_clone_1"]);
    expect(filterVoices(sampleVoices, { accent: "multilingual" }).map((v) => v.id)).toEqual(["fr_julie"]);
  });

  it("filters by search query across name, description, locale, accent, tags", () => {
    expect(filterVoices(sampleVoices, "all", "deep").map((v) => v.id)).toEqual(["am_adam"]);
    expect(filterVoices(sampleVoices, "all", "bonjour").map((v) => v.id)).toEqual([]);
    expect(filterVoices(sampleVoices, "all", "conversational").map((v) => v.id)).toEqual(["fr_julie"]);
    expect(filterVoices(sampleVoices, "all", "British").map((v) => v.id)).toEqual(["custom_clone_1"]);
  });
});
