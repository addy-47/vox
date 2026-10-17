import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  getTransport,
  setTransport,
  invoke,
  invokeWithTimeout,
  invokeWithRetry,
  listen,
  TauriTransport,
  type Transport,
  type UnlistenFn,
} from "../services/transport";

/**
 * Transport carrier layer regression suite.
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT (System Under Test):
 *   The carrier swapper is the global seam every service call routes through;
 *   `invokeWithTimeout` abandons a hung IPC call and surfaces a timeout
 *   rejection; `invokeWithRetry` retries a flaky call with clamped backoff and
 *   rejects with the LAST error, not the first.
 *
 * Production Entry Seam:
 *   setTransport / getTransport / invoke / invokeWithTimeout / invokeWithRetry
 *   (services/transport.ts) — the real dispatch functions every src/services/*
 *   service calls. Swapping in a stub Transport IS the production mechanism
 *   these functions exist to support (a real production constructor path).
 *
 * Direction Check:
 *   Entry is the upstream trigger (service call), not a downstream consumer.
 *
 * Production Path:
 *   service (e.g. pipelineService.engageSession)
 *   → invokeWithTimeout(args) → activeTransport.invoke → Tauri IPC
 *   → observable: resolved value / timeout Error / retry count
 *
 * Observable Exit: resolved value, thrown error type+message, recorded args,
 * retry attempt grouping.
 *
 * Functions in the test: makeStubTransport — a Transport implementation for a
 * seam that exists to be replaced in production. No production code is
 * reimplemented.
 *
 * Phase 2a — Testability 4/4.
 * Phase 2b — False-Green table (each row one-line mutation):
 *   | invoke ignores activeTransport → always raw dispatch  | must fail (args/instance) |
 *   | invokeWithTimeout forgets clearTimeout after resolve  | must fail (timer spy) |
 *   | timeout message omits cmd / ms                        | must fail (message match) |
 *   | invokeWithRetry returns first error → retries missing | must fail (attempt count) |
 *   | backoff clamp at last value removed → NaN delays      | must fail (7 attempt with 1-element backoff) |
 *
 * Phase 4 — Governing sentence:
 *   If the retry loop silently swallowed success values, this test would fail
 *   because the second attempt's value would not be the one returned, and the
 *   invoke call count would exceed 1.
 */

// One module-global reset — tests run in any order.
beforeEach(() => {
  setTransport(new TauriTransport());
  vi.useRealTimers();
});

type InvokeImpl = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

function makeStubTransport(
  invokeImpl: InvokeImpl = () => Promise.resolve(null),
): Transport & { invokes: Array<{ cmd: string; args?: Record<string, unknown> }> } {
  const invokes: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
  return {
    invokes,
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      invokes.push({ cmd, args });
      return invokeImpl(cmd, args) as Promise<T>;
    },
    listen<T>(_event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
      handler({ payload: undefined as T });
      return Promise.resolve(() => {});
    },
    convertFileSrc(filePath: string, protocol?: string): string {
      return `stub://${protocol ?? "asset"}/${filePath}`;
    },
  };
}

describe("carrier swap", () => {
  it("setTransport replaces the active carrier for every routed call", async () => {
    const stub = makeStubTransport((_cmd) => Promise.resolve({ ok: true }));
    setTransport(stub);
    expect(getTransport()).toBe(stub);

    await expect(invoke<{ ok: boolean }>("get_settings")).resolves.toEqual({ ok: true });
    expect(stub.invokes).toEqual([{ cmd: "get_settings", args: undefined }]);
  });

  it("NEGATIVE: routing keeps the original args object identity — no silent reshaping", async () => {
    const stub = makeStubTransport();
    setTransport(stub);
    const args = { patch: { llm: { active: "x" } } } as Record<string, unknown>;
    await invoke("apply_settings_patch", args);
    expect(stub.invokes[0]?.args).toBe(args); // same reference, not a clone
  });
});

describe("invokeWithTimeout", () => {
  it("NEGATIVE: a resolved call must never reject later — the stale deadline keeps firing nothing", async () => {
    vi.useFakeTimers();
    const stub = makeStubTransport(() => Promise.resolve("value"));
    setTransport(stub);

    const pending = invokeWithTimeout("get_settings", undefined, 1000);
    const outcome: Array<"resolved" | "rejected"> = [];
    pending.then(() => outcome.push("resolved")).catch(() => outcome.push("rejected"));

    await vi.advanceTimersByTimeAsync(999);
    await Promise.resolve();
    expect(outcome).toEqual(["resolved"]);
    // The value landed under the deadline: the timer must already be cleared.
    // (After crossing, a fired timer is discarded by the clock, so count must
    // be asserted at this boundary, not later.)
    expect(vi.getTimerCount()).toBe(0);

    // Cross the deadline afterwards: no late rejection may land.
    await vi.advanceTimersByTimeAsync(2000);
    await Promise.resolve();
    expect(outcome).toEqual(["resolved"]);
  });

  it("NEGATIVE: a hung dispatch rejects at the deadline with the cmd and the budget in the message", async () => {
    vi.useFakeTimers();
    const stub = makeStubTransport(() => new Promise<never>(() => {}));
    setTransport(stub);

    const pending = invokeWithTimeout("start_session", { sessionId: 7 }, 250);
    const assertion = expect(pending).rejects.toThrow(/start_session timed out after 250ms/);
    await vi.advanceTimersByTimeAsync(251);
    await assertion;
  });

  it("NEGATIVE: the IPC error itself survives the timeout wrapper", async () => {
    const stub = makeStubTransport(() => Promise.reject(new Error("InvalidState: already started")));
    setTransport(stub);
    await expect(invokeWithTimeout("start_session", undefined, 1000)).rejects.toThrow(
      "InvalidState: already started"
    );
  });
});

describe("invokeWithRetry", () => {
  it("NEGATIVE: retries a flaky call until the last attempt succeeds, returning the final value", async () => {
    let attempt = 0;
    const stub = makeStubTransport(() => {
      attempt += 1;
      return attempt < 3 ? Promise.reject(new Error("transient")) : Promise.resolve("recovered");
    });
    setTransport(stub);

    await expect(
      invokeWithRetry("get_settings", undefined, { retries: 3, backoffMs: [1, 1] })
    ).resolves.toBe("recovered");
    expect(attempt).toBe(3);
    expect(stub.invokes).toHaveLength(3);
  });

  it("NEGATIVE: rejects with the LAST error — the first failure must not be reported as final", async () => {
    let attempt = 0;
    const stub = makeStubTransport(() => {
      attempt += 1;
      return Promise.reject(new Error(`transient-${attempt}`));
    });
    setTransport(stub);

    await expect(invokeWithRetry("x", undefined, { retries: 2, backoffMs: [1, 1] })).rejects.toThrow(
      "transient-3"
    );
    expect(attempt).toBe(3);
  });

  it("NEGATIVE: backoff clamps markers to the last value when the plan is shorter than the retries", async () => {
    const sleepSpy = vi.spyOn(globalThis, "setTimeout");
    let calls = 0;
    const stub = makeStubTransport(() => {
      calls += 1;
      return Promise.reject(new Error("always"));
    });
    setTransport(stub);

    await expect(invokeWithRetry("x", undefined, { retries: 4, backoffMs: [10] })).rejects.toThrow("always");
    expect(calls).toBe(5);
    // Every delay after attempt 0 must be 10ms (the clamp) — a NaN or an
    // undefined delay here hangs the loop and the deadline never fires.
    // Every retry delay must be 10ms (the clamp) — a NaN or an
    // undefined delay here hangs the loop and the deadline never fires.
    // setTimeout is called as setTimeout(r, delay): callback first, ms second.
    const delays = sleepSpy.mock.calls
      .filter(([, ms]) => Number.isFinite(ms as number))
      .map(([, ms]) => ms);
    expect(delays).toEqual([10, 10, 10, 10]);
    sleepSpy.mockRestore();
  });

  it("NEGATIVE: zero retries means exactly one attempt", async () => {
    let calls = 0;
    const stub = makeStubTransport(() => {
      calls += 1;
      return Promise.reject(new Error("boom"));
    });
    setTransport(stub);
    await expect(invokeWithRetry("x", undefined, { retries: 0 })).rejects.toThrow("boom");
    expect(calls).toBe(1);
  });
});

describe("transport event listener plumbing", () => {
  it("listen delivers the handler and returns an unlisten function via the active carrier", async () => {
    let unlistenCalls = 0;
    const stub = {
      listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
        expect(event).toBe("state_changed");
        handler({ payload: { state: "Listening" } as T });
        return Promise.resolve(() => {
          unlistenCalls += 1;
        });
      },
      invoke<T>(_cmd: string, _args?: Record<string, unknown>): Promise<T> {
        return Promise.resolve(null as unknown as T);
      },
      convertFileSrc(filePath: string, protocol?: string): string {
        return `stub://${protocol ?? "asset"}/${filePath}`;
      },
    } as Transport;

    setTransport(stub);
    const received: unknown[] = [];
    const unlisten = await listen<{ state: string }>("state_changed", (e) => received.push(e.payload));
    expect(received).toEqual([{ state: "Listening" }]);

    unlisten();
    expect(unlistenCalls).toBe(1);
  });
});
