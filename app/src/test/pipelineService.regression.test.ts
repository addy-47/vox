import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  waitForPipelineState,
  engageSession,
  disengageSession,
  isDictationOwner,
  isVoxIpcError,
  isInvalidStateError,
  type SessionOrchestrationResult,
} from "@/services/pipelineService";
import { useSessionStore } from "@/store/sessionStore";
import type { InteractionState } from "@/services/eventsService";
import { setTransport, TauriTransport, type Transport, type UnlistenFn } from "@/services/transport";

/**
 * Pipeline session orchestration regression suite (services/pipelineService.ts).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT:
 *   The readiness and recovery layer between UI triggers and the Rust
 *   pipeline: the shared poll (`waitForPipelineState`), engage/disengage
 *   orchestration (start_session/end_session + poll + notify-on-confirmed +
 *   snapshot resync), and the recovery contract for InvalidState errors
 *   (engine reports invalid state → the snapshot is the truth: if it holds a
 *   valid pipeline state, the orchestration reports success).
 *
 * Production Entry Seam:
 *   The real exported functions, driven over the swappable Transport seam
 *   (invoke / invokeWithTimeout with their real timers), exactly as the
 *   tray, Home page, and session store call them. getRuntimeSnapshot answers
 *   from the same transport — no service is mocked, only the wire.
 *
 * Direction Check:
 *   Entry = the real trigger (user pressed engage/stop; continueSession
 *   auto-engagement). Downstream sinks (interactionState store writes via
 *   notifyState) are observed through the notifyState hook — the SUT's own
 *   output parameter, not a faked store.
 *
 * Production Path:
 *   UI engage → engageSession(notifyState)
 *   → invokeWithTimeout("start_session", …, 8s)
 *   → waitForState("Ready") — fast-path on store state, else poll
 *     get_runtime_snapshot → notifyState("Ready") once confirmed
 *   → failure → resyncFromSnapshot → SessionOrchestrationResult
 *
 * Observable Exit: the result object (success/reason), notifyState calls,
 * get_runtime_snapshot poll counts, and invoke args.
 *
 * Phase 2a — Testability 4/4: pure orchestration over the transport seam;
 * fake timers drive the poll loop and the invokeWithTimeout deadline; the
 * session-store fast-path is production state set through the real store.
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | poll returns true on first non-matching snapshot            | must fail |
 *   | deadline check inverted (poll forever / early false)         | must fail |
 *   | InvalidState routes to "error" reason instead of snapshot resync | must fail |
 *   | notified state pushed for an UNconfirmed value ("Thinking")   | must fail |
 *   | fast-path consults polling instead of the store (extra IPC per engage) | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If a stale intermediate state could be pushed to the store, this test
 *   would fail because notifyState would be called with "Thinking" while
 *   the pipeline never confirmed Ready.
 */

let snapshotQueue: Array<Record<string, unknown> | Error> = [];
let calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];

beforeEach(() => {
  snapshotQueue = [];
  calls = [];
  useSessionStore.setState({ interactionState: "Idle" as InteractionState });
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  setTransport(new TauriTransport());
});

/** Installs a transport whose get_runtime_snapshot pops answers in order. */
function installTransport(overrides: Partial<Record<string, (args?: Record<string, unknown>) => unknown>> = {}) {
  const stub: Transport = {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ cmd, args });
      if (cmd === "get_runtime_snapshot") {
        const next = snapshotQueue.shift();
        if (next instanceof Error) return Promise.reject(next);
        return Promise.resolve((next ?? { pipeline_state: "Idle" }) as unknown as T);
      }
      const override = overrides[cmd];
      if (override) {
        const out = override(args);
        return (out instanceof Promise ? out : Promise.resolve(out)) as unknown as Promise<T>;
      }
      return Promise.resolve(null as unknown as T);
    },
    listen(): Promise<UnlistenFn> {
      return Promise.resolve(() => {});
    },
    convertFileSrc(p: string): string {
      return p;
    },
  };
  setTransport(stub);
}

describe("isDictationOwner / IPC error guards", () => {
  it("owner predicate is exact-match on the Dictation owner", () => {
    expect(isDictationOwner("Dictation")).toBe(true);
    expect(isDictationOwner("Assistant")).toBe(false);
    expect(isDictationOwner(undefined)).toBe(false);
  });

  it("NEGATIVE: error shape guards reject arbitrary throwables", () => {
    expect(isVoxIpcError({ error_type: "InvalidState", message: "x" })).toBe(true);
    expect(isVoxIpcError(new Error("plain"))).toBe(false);
    expect(isVoxIpcError("string err")).toBe(false);
    expect(isVoxIpcError(null)).toBe(false);
    expect(
      isInvalidStateError({ error_type: "InvalidState", message: "m" })
    ).toBe(true);
    expect(
      isInvalidStateError({ error_type: "Timeout", message: "m" })
    ).toBe(false);
  });
});

describe("waitForPipelineState — shared readiness poll", () => {
  it("resolves immediately when the first snapshot already matches", async () => {
    installTransport();
    snapshotQueue.push({ pipeline_state: "Ready" });

    const pending = waitForPipelineState("Ready", { timeoutMs: 1000, intervalMs: 10 });
    await Promise.resolve();
    expect(calls.filter((c) => c.cmd === "get_runtime_snapshot")).toHaveLength(1);
    await expect(pending).resolves.toBe(true);
  });

  it("NEGATIVE: a non-matching first snapshot must NOT pass — polling continues", async () => {
    installTransport();
    snapshotQueue.push({ pipeline_state: "Listening" }, { pipeline_state: "Thinking" }, { pipeline_state: "Ready" });

    const pending = waitForPipelineState("Ready", { timeoutMs: 5000, intervalMs: 10 });
    await vi.advanceTimersByTimeAsync(10);
    await Promise.resolve();
    expect(calls.filter((c) => c.cmd === "get_runtime_snapshot").length).toBe(2);
    await vi.advanceTimersByTimeAsync(20);
    await expect(pending).resolves.toBe(true);
    expect(calls.filter((c) => c.cmd === "get_runtime_snapshot")).toHaveLength(3);
  });

  it("NEGATIVE: snapshot rejections are best-effort — the poll survives them until the deadline", async () => {
    installTransport();
    snapshotQueue.push(new Error("probe flake"), new Error("probe flake"), { pipeline_state: "Idle" });

    const pending = waitForPipelineState("Idle", { timeoutMs: 5000, intervalMs: 10 });
    await vi.advanceTimersByTimeAsync(10);
    await vi.advanceTimersByTimeAsync(10);
    await expect(pending).resolves.toBe(true);
  });

  it("NEGATIVE: deadline expiry returns false rather than polling forever", async () => {
    installTransport();
    snapshotQueue.push({ pipeline_state: "Listening" });

    const pending = waitForPipelineState("Ready", { timeoutMs: 50, intervalMs: 10 });
    const assertion = expect(pending).resolves.toBe(false);
    await vi.advanceTimersByTimeAsync(200);
    await assertion;
  });
});

describe("engageSession", () => {
  it("happy path: start_session, poll to Ready, notify exactly the confirmed target", async () => {
    installTransport();
    snapshotQueue.push({ pipeline_state: "Ready" });
    const notifyState = vi.fn<(s: InteractionState) => void>();

    const pending = engageSession(notifyState, 2000, 42);
    await Promise.resolve(); // start_session microtask
    await vi.advanceTimersByTimeAsync(0);

    const result = await pending;
    expect(result).toEqual({ success: true } satisfies SessionOrchestrationResult);
    expect(calls[0]).toEqual({ cmd: "start_session", args: { sessionId: 42 } });
    expect(notifyState).toHaveBeenCalledTimes(1);
    expect(notifyState).toHaveBeenCalledWith("Ready");
  });

  it("NEGATIVE: an intermediate state must never be pushed as confirmed", async () => {
    installTransport();
    snapshotQueue.push({ pipeline_state: "Thinking" }, { pipeline_state: "Ready" });
    const notifyState = vi.fn<(s: InteractionState) => void>();

    const pending = engageSession(notifyState, 5000, null);
    // engageSession's poll uses the default 100ms interval: one advance must
    // cover it, otherwise the loop's sleep timer keeps the promise pending.
    await vi.advanceTimersByTimeAsync(120);
    const result = await pending;

    expect(result).toEqual({ success: true });
    expect(notifyState).toHaveBeenCalledTimes(1);
    expect(notifyState).toHaveBeenCalledWith("Ready");
  });

  it("NEGATIVE: timeout rejection keeps notifyState silent and reports reason timeout with resync", async () => {
    installTransport({
      start_session: () => new Promise<never>(() => {}), // hangs past the deadline
    });
    snapshotQueue.push({ pipeline_state: "Idle" }); // resync sees a valid state
    const notifyState = vi.fn<(s: InteractionState) => void>();

    const pending = engageSession(notifyState, 50);
    const assertion = expect(pending).resolves.toEqual({ success: false, reason: "timeout" });
    await vi.advanceTimersByTimeAsync(60);
    await assertion;
    expect(notifyState).not.toHaveBeenCalled();
  });

  it("InvalidState recovery: a valid post-failure snapshot is success, not error", async () => {
    installTransport({
      start_session: () =>
        Promise.reject({ error_type: "InvalidState", message: "already started" }),
    });
    snapshotQueue.push({ pipeline_state: "Ready" }); // resync: engine IS running
    const notifyState = vi.fn<(s: InteractionState) => void>();

    const result = await engageSession(notifyState, 100);
    expect(result).toEqual({ success: true });
    expect(notifyState).not.toHaveBeenCalled(); // no confirmation happened via poll
  });

  it("InvalidState with an unusable snapshot is an invalid_state failure", async () => {
    installTransport({
      start_session: () =>
        Promise.reject({ error_type: "InvalidState", message: "boom" }),
    });
    snapshotQueue.push({}); // snapshot present but pipeline_state missing
    const result = await engageSession(vi.fn(), 100);
    expect(result).toEqual({ success: false, reason: "invalid_state" });
  });

  it("NEGATIVE: generic errors report reason error and still fail closed", async () => {
    installTransport({
      start_session: () => Promise.reject(new Error("disk on fire")),
    });
    snapshotQueue.push({ pipeline_state: "Listening" }); // snapshot valid but engine NOT confirming engage
    const result = await engageSession(vi.fn(), 100);
    expect(result).toEqual({ success: false, reason: "error" });
  });
});

describe("disengageSession", () => {
  it("happy path: end_session then confirm Idle via the snapshot poll and notify once", async () => {
    installTransport();
    snapshotQueue.push({ pipeline_state: "Idle" });
    const notifyState = vi.fn<(s: InteractionState) => void>();
    // Store NOT Idle: the fast-path must not short-circuit, so the poll path
    // (confirm → notify) is the one under test here.
    useSessionStore.setState({ interactionState: "Speaking" as InteractionState });

    const pending = disengageSession(notifyState, 2000);
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(0);

    expect(await pending).toEqual({ success: true });
    expect(calls[0]).toEqual({ cmd: "end_session", args: undefined });
    expect(notifyState).toHaveBeenCalledTimes(1);
    expect(notifyState).toHaveBeenCalledWith("Idle");
  });

  it("NEGATIVE: no poll confirmation keeps notifyState silent even on end_session success", async () => {
    installTransport({
      end_session: () => Promise.resolve(null),
    });
    // Two non-Idle polls exhaust the deadline; the resync then reads an
    // INVALID snapshot (no pipeline_state) so recovery cannot launder the
    // timeout into a success.
    snapshotQueue.push({ pipeline_state: "Thinking" }, { pipeline_state: "Working" }, {});
    const notifyState = vi.fn<(s: InteractionState) => void>();
    // Store NOT Idle: the fast-path would otherwise confirm without polling.
    useSessionStore.setState({ interactionState: "Thinking" as InteractionState });

    const pending = disengageSession(notifyState, 40);
    const assertion = expect(pending).resolves.toEqual({ success: false, reason: "timeout" });
    await vi.advanceTimersByTimeAsync(120);
    await assertion;
    expect(notifyState).not.toHaveBeenCalled();
  });

  it("recovery precedence: a timeout followed by a VALID snapshot reports success — the snapshot is the truth", async () => {
    installTransport({
      end_session: () => Promise.resolve(null),
    });
    // Both polls miss ("Thinking"), the deadline expires, and only the resync
    // sees Idle — the snapshot is the truth after a failed wait.
    snapshotQueue.push({ pipeline_state: "Thinking" }, { pipeline_state: "Thinking" }, { pipeline_state: "Idle" });
    const notifyState = vi.fn<(s: InteractionState) => void>();
    useSessionStore.setState({ interactionState: "Thinking" as InteractionState });

    const pending = disengageSession(notifyState, 30);
    const assertion = expect(pending).resolves.toEqual({ success: true });
    await vi.advanceTimersByTimeAsync(120);
    await assertion;
    // Recovery does not notify: the UI state sync is driven by the live event
    // stream, not by the resync path.
    expect(notifyState).not.toHaveBeenCalled();
  });

  it("NEGATIVE: InvalidState on end_session resolves from the snapshot instead of crashing", async () => {
    installTransport({
      end_session: () => Promise.reject({ error_type: "InvalidState", message: "not started" }),
    });
    snapshotQueue.push({ pipeline_state: "Idle" });
    const result = await disengageSession(vi.fn(), 100);
    expect(result).toEqual({ success: true });
  });
});

describe("waitForState fast-path (store-trusted confirmation)", () => {
  it("an already-Ready store state skips start_session-poll IPC entirely", async () => {
    installTransport();
    useSessionStore.setState({ interactionState: "Ready" as InteractionState });
    snapshotQueue.push({ pipeline_state: "Idle" }); // would fail the poll if consulted

    const result = await engageSession(vi.fn(), 100);
    expect(result).toEqual({ success: true });
    expect(calls.filter((c) => c.cmd === "get_runtime_snapshot")).toHaveLength(0);
  });
});
