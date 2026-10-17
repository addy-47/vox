import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { useCatalogStore } from "@/store/catalogStore";
import { useSettingsStore } from "@/store/settingsStore";
import { setTransport, TauriTransport, type Transport, type UnlistenFn } from "@/services/transport";
import type { LlmProviderConfig, LlmModelInfo } from "@/store/settingsStore";

/**
 * Remote model catalog load regression suite (store/catalogStore.ts).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT:
 *   loadRemoteModels — the seam that decides WHEN the provider grid may
 *   display models: fetch-key caching (no re-fetch for an unchanged
 *   provider), in-flight coalescing (double-triggered mounts must not double
 *   hit the backend), immediate stale-model clearing when the provider
 *   switches (the previous provider's models must never appear under the new
 *   one), and the cloud "API key required" guard.
 *
 * Production Entry Seam:
 *   useCatalogStore.getState().loadRemoteModels — the real store action the
 *   catalog modals call. The wire below it (listLlmModels →
 *   invoke("list_llm_models")) is driven through the swappable Transport
 *   seam — the same seam production uses — so arg shapes and call counts are
 *   observed on the real boundary, not mocked at the store level.
 *
 * Direction Check:
 *   The store action is the upstream trigger the components call; renderers
 *   are downstream sinks.
 *
 * Production Path:
 *   provider modal / ModelModal mount → loadRemoteModels(providerConfig)
 *   → fetchKey gate → in-flight gate → immediate stale clear
 *   → list_llm_models IPC → set(models | error)
 *
 * Observable Exit: store state (models, loading, error, fetchedKey) between
 * microtasks — including the SYNCHRONOUS stale-clear before the IPC resolves
 * — plus invoke() call counts and args on the transport records.
 *
 * Phase 2a — Testability 4/4: real store, real action, real transport seam.
 * Test isolation needs NO teardown artifact: `remoteModelsInflight` (module
 * global) is drained by the store's own finally-block whenever a request
 * settles — every test awaits its request, so no entry survives a test.
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | immediate `set({remoteModels: []…})` removed → previous provider's models leak into the new provider's grid | must fail |
 *   | fetchKey cache gate removed → every dialog open re-hits IPC | must fail (invoke count) |
 *   | in-flight coalescing removed → double-mount double-IPC | must fail (invoke count) |
 *   | cloud api_key guard removed → keyless cloud fetch fires | must fail |
 *   | failure path sets loading=true forever → skeleton stuck | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the stale-clear before the IPC regressed, this test would fail because
 *   remoteModels would still contain provider A's models right after
 *   loadRemoteModels(providerB) was called, while expecting [].
 */

const SERVER_A: LlmProviderConfig = {
  kind: "server",
  provider_id: "srv-a",
  base_url: "http://10.0.0.1:11434",
  provider_name: "Ollama A",
};

const SERVER_B: LlmProviderConfig = {
  kind: "server",
  provider_id: "srv-b",
  base_url: "http://10.0.0.2:8080",
  provider_name: "vLLM B",
};

const CLOUD_A: LlmProviderConfig = {
  kind: "cloud",
  provider_id: "openrouter",
  base_url: "https://openrouter.ai/v1",
  api_key: "sk-test",
};

function model(id: string): LlmModelInfo {
  return { id, name: id, size_bytes: null, quantization: null, family: null, provider_kind: "server" };
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (err: unknown) => void;
}

function makeDeferred(): Deferred<never> {
  let resolve!: (v: never) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<never>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

let calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
let responses: Record<string, unknown> = {};
let hangCmds: { [k: string]: Deferred<never> } = {};

beforeEach(() => {
  calls = [];
  responses = {};
  hangCmds = {};
  const stub: Transport = {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ cmd, args });
      const hang = hangCmds[cmd];
      if (hang) return hang.promise as unknown as Promise<T>;
      const res = responses[cmd];
      return Promise.resolve((res === undefined ? null : res) as unknown as T);
    },
    listen(): Promise<UnlistenFn> {
      return Promise.resolve(() => {});
    },
    convertFileSrc(p: string): string {
      return p;
    },
  };
  setTransport(stub);
  useCatalogStore.setState({
    remoteModels: [],
    loadingRemoteModels: false,
    remoteModelsError: null,
    remoteModelsFetchedKey: null,
    capabilitiesCache: {},
    capabilitiesCacheError: null,
    probingMap: {},
    modelCatalog: null,
  });
  useSettingsStore.setState({ settings: null, draftSettings: null });
});

afterEach(() => {
  setTransport(new TauriTransport());
  vi.restoreAllMocks();
});

describe("loadRemoteModels — stale-model clearing at provider switch", () => {
  it("NEGATIVE: provider A's models must not survive a switch to provider B, even before B's IPC resolves", async () => {
    responses["list_llm_models"] = [model("qwen")];
    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    expect(useCatalogStore.getState().remoteModels).toHaveLength(1);

    // Provider B's fetch is parked (hangs) — the observable contract is what
    // is in the store in the gap AFTER the switch call, BEFORE the response.
    hangCmds["list_llm_models"] = makeDeferred();
    const pending = useCatalogStore.getState().loadRemoteModels(SERVER_B);

    expect(useCatalogStore.getState().remoteModels).toEqual([]);
    expect(useCatalogStore.getState().loadingRemoteModels).toBe(true);

    hangCmds["list_llm_models"].resolve([model("vllm-1")] as never);
    await pending;
    expect(useCatalogStore.getState().remoteModels).toHaveLength(1);
    expect(useCatalogStore.getState().remoteModels[0]?.id).toBe("vllm-1");
  });

  it("a renamed provider entry reuses the fetch (fetchKey is identity-independent of display name)", async () => {
    responses["list_llm_models"] = [model("same")];
    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    const callsAfterFirst = calls.length;

    // Same kind + provider_id + base_url + key presence → identical fetchKey
    // → cache hit regardless of the display name.
    await useCatalogStore.getState().loadRemoteModels({ ...SERVER_A, provider_name: "renamed-a" });
    expect(calls.length).toBe(callsAfterFirst);
    expect(useCatalogStore.getState().remoteModels).toHaveLength(1);
  });
});

describe("loadRemoteModels — fetchKey cache", () => {
  it("a completed load for the same provider is reused; a switch refetches; force refetches", async () => {
    setTransport({
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        if (cmd === "list_llm_models") {
          // A fresh array each call: identity differs, contents are equal —
          // the cache must reuse by key, not by response identity.
          return Promise.resolve([model("a1")] as unknown as T);
        }
        return Promise.resolve(null as unknown as T);
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(): string {
        return "";
      },
    } as Transport);

    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(1);

    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(1);

    await useCatalogStore.getState().loadRemoteModels(SERVER_A, true);
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(2);

    await useCatalogStore.getState().loadRemoteModels(SERVER_B);
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(3);
  });

  it("NEGATIVE: a failed load must NOT record its fetchKey — the retry path refetches", async () => {
    let fail = true;
    const stub: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        if (cmd === "list_llm_models") {
          return fail ? Promise.reject(new Error("ECONNREFUSED")) : Promise.resolve([model("retry")] as unknown as T);
        }
        return Promise.resolve(null as unknown as T);
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(): string {
        return "";
      },
    } as Transport;
    setTransport(stub);

    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    const state = useCatalogStore.getState();
    expect(state.remoteModelsError).toBe("ECONNREFUSED");
    expect(state.loadingRemoteModels).toBe(false);

    fail = false;
    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    const after = useCatalogStore.getState();
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(2);
    expect(after.remoteModelsError).toBeNull();
    expect(after.remoteModels).toHaveLength(1);
    expect(after.remoteModelsFetchedKey).not.toBeNull();
  });
});

describe("loadRemoteModels — in-flight coalescing", () => {
  it("NEGATIVE: two concurrent loads for one provider must hit IPC exactly once", async () => {
    hangCmds["list_llm_models"] = makeDeferred();
    const p1 = useCatalogStore.getState().loadRemoteModels(CLOUD_A);
    const p2 = useCatalogStore.getState().loadRemoteModels(CLOUD_A);

    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(1);

    hangCmds["list_llm_models"].resolve([model("cloud-1"), model("cloud-2")] as never);
    await Promise.all([p1, p2]);
    expect(useCatalogStore.getState().remoteModels).toHaveLength(2);
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(1);
    expect(useCatalogStore.getState().loadingRemoteModels).toBe(false);
  });

  it("a second caller observing an in-flight failure must not report an error set by itself", async () => {
    const deferred = makeDeferred();
    hangCmds["list_llm_models"] = deferred;
    const owner = useCatalogStore.getState().loadRemoteModels(CLOUD_A);
    const follower = useCatalogStore.getState().loadRemoteModels(CLOUD_A);

    deferred.reject(new Error("502 from provider"));
    await owner;
    await follower;
    expect(useCatalogStore.getState().remoteModelsError).toBe("502 from provider");
  });
});

describe("loadRemoteModels — guards", () => {
  it("NEGATIVE: a cloud provider without an API key is rejected in the store, never sent to IPC", async () => {
    await useCatalogStore.getState().loadRemoteModels({ ...CLOUD_A, api_key: "   " });
    const state = useCatalogStore.getState();
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(0);
    expect(state.remoteModelsError).toMatch(/API key required/i);
    expect(state.loadingRemoteModels).toBe(false);
  });

  it("embedded/local selection clears any remote list and never keys a fetch", async () => {
    responses["list_llm_models"] = [model("qwen")];
    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    expect(useCatalogStore.getState().remoteModels).toHaveLength(1);

    await useCatalogStore.getState().loadRemoteModels({ kind: "embedded" });
    const state = useCatalogStore.getState();
    expect(calls.filter((c) => c.cmd === "list_llm_models")).toHaveLength(1);
    expect(state.remoteModels).toEqual([]);
    expect(state.remoteModelsError).toBeNull();
    expect(state.loadingRemoteModels).toBe(false);
  });

  it("NEGATIVE: loading must settle false on IPC failure — a stuck skeleton is a regression", async () => {
    const stub: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        return Promise.reject(new Error("backend crash")) as unknown as Promise<T>;
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(): string {
        return "";
      },
    } as Transport;
    setTransport(stub);

    const pending = useCatalogStore.getState().loadRemoteModels(SERVER_A);
    expect(useCatalogStore.getState().loadingRemoteModels).toBe(true);
    await pending;
    expect(useCatalogStore.getState().loadingRemoteModels).toBe(false);
    expect(useCatalogStore.getState().remoteModelsError).toContain("backend crash");
  });
});

describe("clearRemoteModels / clearRemoteModelsError", () => {
  it("clear wipes models and error but keeps the fetch key (re-open reuses cache)", async () => {
    responses["list_llm_models"] = [model("qwen")];
    await useCatalogStore.getState().loadRemoteModels(SERVER_A);
    const keyBefore = useCatalogStore.getState().remoteModelsFetchedKey;
    expect(keyBefore).not.toBeNull();

    useCatalogStore.getState().clearRemoteModels();
    expect(useCatalogStore.getState()).toMatchObject({ remoteModels: [], remoteModelsError: null, loadingRemoteModels: false });
    expect(useCatalogStore.getState().remoteModelsFetchedKey).toBe(keyBefore);
  });

  it("clearRemoteModelsError only acts when an error exists", async () => {
    useCatalogStore.setState({ remoteModelsError: null, remoteModelsFetchedKey: "k" });
    useCatalogStore.getState().clearRemoteModelsError();
    expect(useCatalogStore.getState().remoteModelsFetchedKey).toBe("k");

    useCatalogStore.setState({ remoteModelsError: "e" });
    useCatalogStore.getState().clearRemoteModelsError();
    expect(useCatalogStore.getState().remoteModelsError).toBeNull();
  });
});

describe("capabilities cache", () => {
  it("loadCapabilitiesCache maps cached_map/cache_error from the wire verbatim", async () => {
    responses["get_model_capabilities_cache"] = { cached_map: { "server:model-a": { x: 1 } }, cache_error: null };
    await useCatalogStore.getState().loadCapabilitiesCache();
    const state = useCatalogStore.getState();
    expect(state.capabilitiesCache).toEqual({ "server:model-a": { x: 1 } });
    expect(state.capabilitiesCacheError).toBeNull();
  });

  it("NEGATIVE: a cache failure surfaces cache_error without throwing", async () => {
    const stub: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        return Promise.reject(new TypeError("no such backend")) as unknown as Promise<T>;
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(): string {
        return "";
      },
    } as Transport;
    setTransport(stub);

    await useCatalogStore.getState().loadCapabilitiesCache();
    expect(useCatalogStore.getState().capabilitiesCacheError).toContain("no such backend");
    expect(calls[0]?.cmd).toBe("get_model_capabilities_cache");
  });
});
