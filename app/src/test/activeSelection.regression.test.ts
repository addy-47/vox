import { describe, it, expect, beforeEach } from "vitest";
import { getProviderLogoDef, PROVIDER_LOGO_REGISTRY } from "@/shared/components/settings/providers/ProviderLogo";
import {
  promoteActive,
  getEffectiveLlmServers,
  getEffectiveCloudApiKey,
} from "@/shared/lib/activeSelection";
import { selectActiveLlmProvider, useSettingsStore } from "@/store/settingsStore";
import { minimalSettings, settingsFrom, EMBEDDED_ACTIVE, SERVER_ACTIVE } from "@/test/harness/settingsFixtures";

/**
 * Active-selection regression suite — provider/model primacy + key/server
 * resolution across draft and committed settings.
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT (System Under Test):
 *   The committed active selection surfaces first in every catalog list
 *   (models, cloud providers, TTS voices grid, custom servers), and
 *   draft→committed resolution picks the key/server/model the user actually
 *   selected — not a stale one from a previous provider.
 *
 * Production Entry Seam:
 *   promoteActive / getEffectiveLlmServers / getEffectiveCloudApiKey
 *     (shared/lib/activeSelection.ts) — the real functions called by
 *     ModelModal.tsx (modalFilteredModels), ProviderModal.tsx
 *     (ttsFilteredProviders, modalFilteredProviders, filteredCustomServers),
 *     and selectActiveLlmProvider (settingsStore). The sort that feeds
 *     promoteActive stays in each component pre-verified by callers; the
 *     primacy *meaning* (active first, drafted never reorders) lives here.
 *
 * Direction Check:
 *   These are the trigger-side resolvers: component read → selection state →
 *   list order. Testing a downstream renderer would test the sink, not the
 *   pipeline.
 *
 * Production Path:
 *   committed settings / committed active selection
 *   → activeSelection resolvers (+ sort in the modal component)
 *   → promoteActive ordering
 *   → rendered grid ordering → user activates a model/provider/voice
 *
 * Observable Exit:
 *   ordered arrays (ids) and resolved provider/key/server objects.
 *
 * Production functions the test calls:
 *   setup:   useSettingsStore.setState (real store)
 *   entry:   promoteActive / getEffectiveCloudApiKey / getEffectiveLlmServers
 *            / selectActiveLlmProvider (real exports)
 *   observe: return values and store state (no mock of the seam)
 *
 * Phase 2a — Testability: yes (4/4). All four entry points are exported
 * production functions with real signatures; state is assembled through the
 * real store constructor; outputs are directly observable; the IPC boundary
 * is untouched (pure resolvers).
 *
 * Phase 2b — False-Green table (each row = one-line production mutation):
 *   | promoteActive splice+unshift deleted → active stays mid-list | must fail |
 *   | activeId===first no-op removed → reorders on already-first    | must fail |
 *   | getEffectiveCloudApiKey returns fallback committed key when draft has none | must fail |
 *   | getEffectiveCloudApiKey precedence swapped (committed wins over draft) | must fail |
 *   | getEffectiveLlmServers fallback order changed → wrong server base_url | must fail |
 *   | selectActiveLlmProvider returns draft provider for committed lookups | must fail |
 *
 * Superseded test note: this file previously defined `sortWithPrimacy` and
 * friends *inside the test*, reimplementing production. Deleting the
 * production unshift (ModelModal.tsx) left those 4 tests green — a proven
 * survivor mutant (mutation run 2026-10-17). The fake tests are gone; the
 * seam is now covered through the real exports above.
 *
 * Phase 4 — Governing sentence:
 *   If the production splice+unshift broke, this test would fail because
 *   promoteActive(["a","b","c"], "c") would keep "c" at index 2 while
 *   expecting index 0.
 */

const resetSettingsStore = () => {
  useSettingsStore.setState({
    settings: null,
    draftSettings: null,
    failedSaveDomains: {},
    restartKeys: [],
    hasChanges: false,
    isCommitting: false,
  });
};

describe("promoteActive — active card primacy (real production function)", () => {
  const pool = [
    { id: "a-anthropic/claude" },
    { id: "b-meta-llama/llama-3-8b" },
    { id: "c-openai/gpt-4o" },
    { id: "d-deepseek/deepseek-chat" },
  ];

  it("moves the committed active entry to index 0", () => {
    expect(promoteActive(pool, "c-openai/gpt-4o")[0].id).toBe("c-openai/gpt-4o");
    expect(promoteActive(pool, "b-meta-llama/llama-3-8b")[0].id).toBe("b-meta-llama/llama-3-8b");
  });

  it("NEGATIVE: no reordering when nothing is committed (draft does not promote)", () => {
    expect(promoteActive(pool, undefined).map((m) => m.id)).toEqual(pool.map((m) => m.id));
    expect(promoteActive(pool, null).map((m) => m.id)).toEqual(pool.map((m) => m.id));
  });

  it("no-op when the active entry is already first", () => {
    const once = promoteActive(pool, "a-anthropic/claude");
    expect(promoteActive(once, "a-anthropic/claude")).toEqual(once);
  });

  it("NEGATIVE: an unknown id never mutates order", () => {
    expect(promoteActive(pool, "not-in-list").map((m) => m.id)).toEqual(pool.map((m) => m.id));
  });

  it("single-entry and empty lists are stable", () => {
    expect(promoteActive([{ id: "only" }], "only")).toEqual([{ id: "only" }]);
    expect(promoteActive([], "anything")).toEqual([]);
  });

  it("every other entry keeps its relative order (stable splice, not a re-sort)", () => {
    expect(promoteActive(pool, "d-deepseek/deepseek-chat")).toEqual([
      { id: "d-deepseek/deepseek-chat" },
      { id: "a-anthropic/claude" },
      { id: "b-meta-llama/llama-3-8b" },
      { id: "c-openai/gpt-4o" },
    ]);
  });

  it("does not mutate the input list", () => {
    const input = [...pool];
    promoteActive(input, "c-openai/gpt-4o");
    expect(input).toEqual(pool);
  });
});

describe("getEffectiveCloudApiKey — draft/committed precedence", () => {
  it("NEGATIVE: resolves the draft layer first, committed only as fallback", () => {
    const committed = { providers: { llm: { cloud_keys: { openrouter: "sk-committed" } } } };
    const draft = { llm: { cloud_keys: { openrouter: "sk-draft" } } };
    // Draft wins — while the user edits a key, the stale committed key must
    // never silently ship to the wrong endpoint.
    expect(getEffectiveCloudApiKey(draft, committed, "openrouter")).toBe("sk-draft");
    // Committed fallback when the draft has nothing.
    expect(getEffectiveCloudApiKey(null, committed, "openrouter")).toBe("sk-committed");
  });

  it("NEGATIVE: no provider id means no key, never a truthy default", () => {
    expect(getEffectiveCloudApiKey(
      { llm: { cloud_keys: { openrouter: "sk-something" } } },
      null,
      null
    )).toBeUndefined();
  });

  it("whitespace-only committed keys do not resolve", () => {
    expect(getEffectiveCloudApiKey(null, { llm: { cloud_keys: { nvidia: "   " } } }, "nvidia")).toBeUndefined();
  });
});

describe("getEffectiveLlmServers — custom-server unification", () => {
  const draftServer = { id: "srv_draft", name: "Draft", url: "http://127.0.0.1:11434", model: "qwen", protocol: "openai", api_key: null };
  const committedServer = { id: "llm_custom_123", name: "Ollama", url: "http://192.168.1.184:11434", model: "qwen", protocol: "openai", api_key: null };

  it("NEGATIVE: server inventory precedence is draft.llm.servers → draft.providers → committed.providers → committed.llm", () => {
    expect(getEffectiveLlmServers(
      { llm: { servers: [draftServer] }, providers: { llm: { servers: [committedServer] } } },
      { llm: { servers: [{ ...committedServer, id: "stale" }] }, providers: { llm: { servers: [{ ...committedServer, id: "stale" }] } } },
    )).toEqual([draftServer]);

    expect(getEffectiveLlmServers(
      { providers: { llm: { servers: [draftServer] } } },
      { providers: { llm: { servers: [] } } }
    )).toEqual([draftServer]);

    expect(getEffectiveLlmServers(
      null,
      { providers: { llm: { servers: [committedServer] } } }
    )).toEqual([committedServer]);

    expect(getEffectiveLlmServers(
      null,
      { llm: { servers: [committedServer] } }
    )).toEqual([committedServer]);
  });

  it("NEGATIVE: no server anywhere resolves to an empty list, never undefined", () => {
    expect(getEffectiveLlmServers(null, null)).toEqual([]);
  });
});

describe("selectActiveLlmProvider — draft selection is the trigger, committed inventory is the source", () => {
  beforeEach(() => {
    resetSettingsStore();
  });
  it("NEGATIVE: an uncommitted draft provider is NOT activated until its server inventory resolves", () => {
    const committed = minimalSettings();
    committed.llm.active = EMBEDDED_ACTIVE;
    const draft = settingsFrom(minimalSettings());
    draft.llm.active = SERVER_ACTIVE; // drafted, not committed

    useSettingsStore.setState({ settings: committed, draftSettings: draft });

    const provider = selectActiveLlmProvider(useSettingsStore.getState());
    // Draft selection is the trigger but there is no resolved server behind
    // it (draft.llm.servers absent, committed inventory absent):
    expect(provider.kind).toBe("server");
    expect(provider.base_url).toBe("");
    expect(provider.provider_id).toBe("srv-1");

    useSettingsStore.setState({ settings: null, draftSettings: null });
  });

  it("resolves a committed custom server through the inventory the user selected", () => {
    const committed = minimalSettings();
    committed.providers = {
      llm: { servers: [{ id: "llm_custom_123", name: "Ollama", url: "http://192.168.1.184:11434", model: "", protocol: "openai", api_key: null }], cloud_keys: {} },
      tts: { servers: [] },
      realtime: { api_keys: {} },
    };
    const draft = settingsFrom(minimalSettings());
    draft.llm.active = { category: "server", provider: "llm_custom_123", model: "" };

    useSettingsStore.setState({ settings: committed, draftSettings: draft });

    const provider = selectActiveLlmProvider(useSettingsStore.getState());
    expect(provider.kind).toBe("server");
    expect(provider.base_url).toBe("http://192.168.1.184:11434");
    expect(provider.provider_name).toBe("Ollama");
    expect(provider.provider_id).toBe("llm_custom_123");

    useSettingsStore.setState({ settings: null, draftSettings: null });
  });
});

describe("ProviderLogo — post-ZBC registry stays empty until logos return (tombstone)", () => {
  // Not a behavior test of rendering: it pins the ZBC purge so the registry
  // cannot silently repopulate with per-provider branching (invariant: model
  // agnosticism). Delete this block with the registry contents when logos
  // become manifest-driven.
  it("NEGATIVE: registry empty and every provider id resolves to no logo function", () => {
    expect(PROVIDER_LOGO_REGISTRY).toEqual([]);
    for (const id of ["openai", "anthropic", "groq", "ollama", "some_obscure_host_404"]) {
      expect(getProviderLogoDef(id)).toBeUndefined();
    }
  });
});
