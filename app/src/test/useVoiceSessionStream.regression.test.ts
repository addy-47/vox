import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  useVoiceSessionStream,
  type VoiceSessionStreamHandlers,
  type VoiceSessionHydration,
} from "@/shared/hooks/useVoiceSessionStream";
import { useSessionStore } from "@/store/sessionStore";
import { setTransport, TauriTransport, type Transport, type UnlistenFn } from "@/services/transport";
import type { TranscriptPayload, TurnMetricsPayload, StateChangedPayload } from "@/services/eventsService";
import { minimalSettings, settingsFrom } from "@/test/harness/settingsFixtures";
import type { TurnRow } from "@/services/persistenceService";

/**
 * Voice session event-stream regression suite (shared/hooks/useVoiceSessionStream.ts).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT:
 *   The single lifecycle hook consolidating the session event stream —
 *   subscription fan-in (state_changed, transcript partial/final, llm_token,
 *   settings-updated, turn metrics), dictation-owner isolation, turn-boundary
 *   commit flushing on turn_id change, 30ms commit throttling, and the
 *   first-mount hydration snapshot (settings + runtime snapshot + persisted
 *   turns).
 *
 * Production Entry Seam:
 *   The hook as mounted by the voice pages; events are delivered through the
 *   REAL subscription path — eventsService.on → transport.listen. The capture
 *   transport records the exact handlers production registers and delivers
 *   `{ payload }` envelopes on demand: the same wire shape the backend emits.
 *   Hydration inputs arrive over the same wire (get_settings /
 *   get_runtime_snapshot / get_turns).
 *
 * Direction Check:
 *   The hook IS the event-driven trigger layer — emitted payloads and mount
 *   are the real inputs; there is no deeper seam to drive.
 *
 * Production Path:
 *   backend event → Tauri event envelope → eventsService.on(handler)
 *   → transport.listen(event, wrapper) → useVoiceSessionStream subscriber
 *   → throttle timers (30ms) → transcript / assistantText state
 *   → turn_id boundary → commitTurn → onTurnComplete
 *
 * Observable Exit: hook state (transcript, assistantText), handler calls
 * (onStateChanged / onTurnComplete / onSettingsUpdated / onTurnMetrics /
 * onHydrated), and the hydration payload assembled from three IPC reads.
 *
 * Phase 2a — Testability 4/4: no mocking of the SUT or its services; only
 * the swappable Transport seam carries the events. jsdom + fake timers cover
 * the throttle windows. persistenceService's module-global turns cache is
 * avoided by using a distinct session id per hydration test.
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | isDictationOwner guards removed → dictation transcript floods the voice UI | must fail |
 *   | state_changed validation bare → invalid states pushed → crash downstream | must fail |
 *   | turn-boundary commitTurn call removed → previous turn's text leaks into the next turn | must fail |
 *   | llm_token accumulation flush never resolves → assistantText stuck empty | must fail |
 *   | hydration slice(-100) or max-id arithmetic wrong → history drift | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the dictation guard regressed, this test would fail because a
 *   transcript_partial emitted with owner "Dictation" would end up visible in
 *   the voice session transcript, while expecting the empty string "".
 */

type Handler = (event: { payload: unknown }) => void;
let eventHandlers: Map<string, Set<Handler>> = new Map();
let calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
let responses: Record<string, unknown> = {};

beforeEach(() => {
  eventHandlers = new Map();
  calls = [];
  responses = {};
  useSessionStore.setState({ interactionState: "Idle" });
  const stub: Transport = {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ cmd, args });
      return Promise.resolve((responses[cmd] ?? null) as unknown as T);
    },
    listen<_Evt>(event: string, capture: (event: { payload: _Evt }) => void): Promise<UnlistenFn> {
      if (!eventHandlers.has(event)) eventHandlers.set(event, new Set());
      eventHandlers.get(event)!.add(capture as unknown as Handler);
      return Promise.resolve(() => {
        eventHandlers.get(event)?.delete(capture as unknown as Handler);
      });
    },
    convertFileSrc(p: string): string {
      return p;
    },
  };
  setTransport(stub);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  setTransport(new TauriTransport());
});

/** Delivers ONE backend event envelope to every live handler of an event. */
function emit(event: string, payload: unknown): void {
  const handlers = eventHandlers.get(event);
  expect(handlers?.size ?? 0, `no subscriber for ${event}`).toBeGreaterThan(0);
  act(() => {
    handlers?.forEach((h) => h({ payload }));
  });
}

const DICTATION: TranscriptPayload["owner"] = "Dictation";
const ASSISTANT: TranscriptPayload["owner"] = "Assistant";

function mount(): {
  result: { current: ReturnType<typeof useVoiceSessionStream> };
  onStateChanged: ReturnType<typeof vi.fn>;
  onSettingsUpdated: ReturnType<typeof vi.fn>;
  onTurnComplete: ReturnType<typeof vi.fn>;
  onHydrated: ReturnType<typeof vi.fn>;
  onTurnMetrics: ReturnType<typeof vi.fn>;
  flushHydration: () => Promise<void>;
  unmount: () => void;
} {
  const onStateChanged = vi.fn<(p: StateChangedPayload) => void>();
  const onSettingsUpdated = vi.fn<() => void>();
  const onTurnComplete = vi.fn<(turn: { user: string; assistant: string }) => void>();
  const onHydrated = vi.fn<(h: VoiceSessionHydration) => void>();
  const onTurnMetrics = vi.fn<(p: TurnMetricsPayload) => void>();

  const handlers: VoiceSessionStreamHandlers = {
    onStateChanged,
    onSettingsUpdated,
    onTurnComplete,
    onHydrated,
    onTurnMetrics,
  };
  const utils = renderHook(() => useVoiceSessionStream(handlers));

  // Hydration is scheduled with setTimeout(hydrate, 0): advance then flush.
  const flushHydration = async () => {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
  };

  return {
    result: utils.result,
    onStateChanged,
    onSettingsUpdated,
    onTurnComplete,
    onHydrated,
    onTurnMetrics,
    flushHydration,
    unmount: utils.unmount,
  };
}

describe("transcript_partial — dictation isolation and throttling", () => {
  it("NEGATIVE: dictation-owned partials must never reach the voice transcript", async () => {
    const h = mount();
    emit("transcript_partial", { turn_id: 7, text: "dictated words", owner: DICTATION });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(35);
    });
    expect(h.result.current.transcript).toBe("");
  });

  it("staff-owned partials land in transcript after the 30ms commitment window", async () => {
    const h = mount();
    emit("transcript_partial", { turn_id: 7, text: "you hear me", owner: ASSISTANT });
    expect(h.result.current.transcript).toBe(""); // not committed yet
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30);
    });
    expect(h.result.current.transcript).toBe("you hear me");
  });

  it("NEGATIVE: 30ms throttle — bursts of partials commit one render at the window edge", async () => {
    const h = mount();
    emit("transcript_partial", { turn_id: 7, text: "a", owner: ASSISTANT });
    for (const t of ["ab", "abc", "abcd"]) {
      emit("transcript_partial", { turn_id: 7, text: t, owner: ASSISTANT });
    }
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30);
    });
    expect(h.result.current.transcript).toBe("abcd");
    expect(eventHandlers.get("transcript_partial")?.size).toBe(1);
  });
});

describe("turn boundaries — commit flush on turn_id change", () => {
  it("NEGATIVE: a different turn_id mid-stream commits the PREVIOUS turn with its user+assistant text", async () => {
    const h = mount();
    // Turn 1: user text arrives, then its assistant response streams.
    emit("transcript_final", { turn_id: 11, text: "who is there", owner: ASSISTANT });
    emit("llm_token", { turn_id: 11, token: "a friend" });

    // Turn 2 opens — the in-flight turn 11 must flush as a complete turn.
    emit("llm_token", { turn_id: 12, token: "new" });

    expect(h.onTurnComplete).toHaveBeenCalledTimes(1);
    expect(h.onTurnComplete).toHaveBeenCalledWith({
      id: 11,
      user: "who is there",
      assistant: "a friend",
    });
  });

  it("NEGATIVE: llm tokens accumulate per turn and commit-flush on the next turn boundary", async () => {
    const h = mount();
    emit("llm_token", { turn_id: 3, token: "Hel" });
    emit("llm_token", { turn_id: 3, token: "lo, " });
    emit("llm_token", { turn_id: 3, token: "world" });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30);
    });
    expect(h.result.current.assistantText).toBe("Hello, world");

    emit("llm_token", { turn_id: 4, token: "next" });
    expect(h.onTurnComplete).toHaveBeenCalledWith({
      id: 3,
      user: "",
      assistant: "Hello, world",
    });
  });

  it("manual commitTurn flushes the active turn without a boundary event", () => {
    const h = mount();
    emit("transcript_final", { turn_id: 5, text: "ask", owner: ASSISTANT });
    emit("llm_token", { turn_id: 5, token: "ans" });

    act(() => {
      h.result.current.commitTurn();
    });
    expect(h.onTurnComplete).toHaveBeenCalledTimes(1);
    expect(h.result.current.transcript).toBe("");
    expect(h.result.current.assistantText).toBe("");
  });

  it("clearTranscript drops the active turn without flushing it", () => {
    const h = mount();
    emit("transcript_final", { turn_id: 5, text: "dropped", owner: ASSISTANT });

    act(() => {
      h.result.current.clearTranscript();
    });
    expect(h.onTurnComplete).not.toHaveBeenCalled();
    expect(h.result.current.transcript).toBe("");
  });
});

describe("state_changed — validation and ownership", () => {
  it("NEGATIVE: an unknown state value must never reach handlers", () => {
    const h = mount();
    emit("state_changed", { owner: ASSISTANT, state: "Bogus", turn_id: 1 });
    expect(h.onStateChanged).not.toHaveBeenCalled();
  });

  it("negates only the dictation owner: session owner states pass through", () => {
    const h = mount();
    emit("state_changed", { owner: ASSISTANT, state: "Listening", turn_id: 1 });
    expect(h.onStateChanged).toHaveBeenCalledWith({ owner: ASSISTANT, state: "Listening", turn_id: 1 });

    emit("state_changed", { owner: DICTATION, state: "Listening", turn_id: 2 });
    expect(h.onStateChanged).toHaveBeenCalledTimes(1);
  });

  it("turn_metrics and settings_updated pass through the single subscription set", () => {
    const h = mount();
    emit("turn_metrics", { turn_id: 1, ttft_ms: 40, ttfa_ms: 120, total_voice_latency_ms: 500, context_tokens_used: 2, context_window: 8192 });
    expect(h.onTurnMetrics).toHaveBeenCalledTimes(1);

    emit("settings-updated", null);
    expect(h.onSettingsUpdated).toHaveBeenCalledTimes(1);
  });
});

describe("hydration — settings + runtime snapshot + persisted turns", () => {
  it("NEGATIVE: a running conversation hydrates session id and turn history (windowed to 100, max turn id counter)", async () => {
    const settings = settingsFrom(minimalSettings());
    // Defensive lowercasing path: the wire may carry display-case values even
    // though the typed fixture is lowercase-only (same cast shape the hook does).
    (settings.interaction.pipeline_mode = "REALTIME" as unknown as typeof settings.interaction.pipeline_mode);
    const turns: TurnRow[] = Array.from({ length: 120 }, (_, i) => ({
      id: i + 1,
      session_id: 777,
      turn_id: i + 1,
      user_text: `u${i + 1}`,
      assistant_text: `a${i + 1}`,
      created_at: 0,
    }));
    responses["get_settings"] = { settings, models_dir_exists: true, settings_path: "/x" };
    responses["get_runtime_snapshot"] = { pipeline_state: "Ready", conversation_id: 777, cpu_governor: "performance", cpu_governor_optimal: false };
    responses["get_turns"] = turns;

    const h = mount();
    await h.flushHydration();

    expect(h.onHydrated).toHaveBeenCalledTimes(1);
    const payload = h.onHydrated.mock.calls[0]?.[0];
    expect(payload.interactionMode).toBe("passive");
    expect(payload.pipelineMode).toBe("realtime");
    expect(payload.cpuWarning).toEqual({ governor: "performance" });
    expect(payload.activeSessionId).toBe(777);
    expect(payload.dialogueHistory).toHaveLength(100);
    expect(payload.dialogueHistory[0]?.user).toBe("u21"); // last 100 of 120
    expect(payload.dialogueHistory[99]?.assistant).toBe("a120");
    expect(payload.turnIdCounter).toBe(120);
  });

  it("NEGATIVE: no live conversation hydrates without touching the turns IPC", async () => {
    responses["get_settings"] = { settings: minimalSettings(), models_dir_exists: true, settings_path: "/x" };
    responses["get_runtime_snapshot"] = { pipeline_state: "Idle", conversation_id: 0, cpu_governor: "sched_util", cpu_governor_optimal: true };

    const h = mount();
    await h.flushHydration();

    expect(h.onHydrated).toHaveBeenCalledWith(
      expect.objectContaining({ activeSessionId: null, dialogueHistory: [], turnIdCounter: 0, cpuWarning: null })
    );
    expect(calls.some((c) => c.cmd === "get_turns")).toBe(false);
  });
});

describe("unmount discipline", () => {
  it("NEGATIVE: unmounted hook drops all events and unsubscribes its listeners", async () => {
    const h = mount();
    expect(eventHandlers.get("transcript_partial")?.size).toBe(1);
    h.unmount();
    // Unsubscription is eventual: eventsService registers the unlisten when
    // the listen promise resolves, and a pre-registration unmount flips its
    // cancelled flag. One microtask flush settles the subscription bookkeeping.
    await Promise.resolve();
    expect(eventHandlers.get("transcript_partial")?.size ?? 0).toBe(0);

    // Late events after unmount: no state writes (React act would surface
    // illegal updates) and no handler calls.
    expect(() => emitSafe()).not.toThrow();
    function emitSafe(): void {
      const handlers = eventHandlers.get("transcript_partial");
      handlers?.forEach((fn) => fn({ payload: { turn_id: 9, text: "late", owner: ASSISTANT } }));
    }
    expect(h.result.current.transcript).toBe("");
  });

  it("NEGATIVE: a throttle timer pending at unmount must not fire into a dead component", async () => {
    const h = mount();
    emit("transcript_partial", { turn_id: 7, text: "pending", owner: ASSISTANT });
    h.unmount();
    // The committed fire would React-error in act() if it still ran setState.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(40);
    });
    expect(h.result.current.transcript).toBe("");
  });
});
