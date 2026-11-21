import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup, act, fireEvent } from "@testing-library/react";
import {
  VoiceSessionProvider,
  useVoiceSessionActions,
  type VoiceSessionActionsContextValue,
} from "@/shared/context/VoiceSessionContext";
import { useSessionStore } from "@/store/sessionStore";
import { setTransport, TauriTransport, type Transport, type UnlistenFn } from "@/services/transport";
import type { InteractionState } from "@/services/eventsService";

/**
 * Disengage orchestration DOM suite (VoiceSessionContext.executeDisengage).
 *
 * Black-box scene contract:
 *   Entry trigger  : the real `disengage` action exposed by VoiceSessionProvider,
 *                    driven over the swappable transport wire. No service is mocked.
 *   Observable exit: the `end_session` IPC args, the ConfirmModal DOM, and the
 *                    real zustand session state after the orchestration settles.
 *   Trajectory     : Working -> (pre-check) modal, or Working -> end_session(force)
 *                    -> confirmed Idle -> local session state cleared.
 *
 * False-green table:
 *   | local state cleared before confirmation              | must fail |
 *   | end_session force flag ignored                       | must fail |
 *   | active-task pre-check skipped                        | must fail |
 *   | modal confirm calls force=false                      | must fail |
 *
 * Governing sentence: if executeDisengage cleared local state on a rejected
 * or unconfirmed end_session, the "state preserved" test would fail because
 * activeSessionId and transcript would be empty instead of the seeded values.
 */

let calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
let responses: Record<string, unknown> = {};
let actions: VoiceSessionActionsContextValue | null = null;

function ActionsProbe() {
  actions = useVoiceSessionActions();
  return null;
}

beforeEach(() => {
  calls = [];
  responses = {};
  actions = null;
  useSessionStore.setState({
    interactionState: "Idle" as InteractionState,
    activeSessionId: null,
    transcript: "",
    assistantText: "",
    dialogueHistory: [],
    isLaunching: false,
  });
  const stub: Transport = {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ cmd, args });
      return Promise.resolve((responses[cmd] ?? null) as unknown as T);
    },
    listen(): Promise<UnlistenFn> {
      return Promise.resolve(() => {});
    },
    convertFileSrc(p: string): string {
      return p;
    },
  };
  setTransport(stub);
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.clearAllTimers();
  vi.useRealTimers();
  setTransport(new TauriTransport());
});

async function mountProvider(): Promise<void> {
  render(
    <VoiceSessionProvider>
      <ActionsProbe />
    </VoiceSessionProvider>,
  );
  // Hydration is scheduled with setTimeout(hydrate, 0); settle it before seeding.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
}

function seedActiveSession(): void {
  useSessionStore.setState({
    interactionState: "Working" as InteractionState,
    activeSessionId: 42,
    transcript: "keep-transcript",
    assistantText: "keep-assistant",
    dialogueHistory: [{ user: "u", assistant: "a", id: 1 }],
  });
}

describe("VoiceSessionContext disengage orchestration", () => {
  it("NEGATIVE: an unconfirmed end_session(force=false) preserves local session state", async () => {
    responses["get_active_tasks_count"] = 0;
    responses["end_session"] = null;
    // Valid but non-Idle snapshots never confirm the teardown.
    responses["get_runtime_snapshot"] = { pipeline_state: "Working" };
    await mountProvider();
    seedActiveSession();

    let pending!: Promise<void>;
    act(() => {
      pending = actions!.disengage();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(8200);
      await pending;
    });

    expect(calls.filter((c) => c.cmd === "end_session")).toEqual([
      { cmd: "end_session", args: { force: false } },
    ]);
    const state = useSessionStore.getState();
    expect(state.activeSessionId).toBe(42);
    expect(state.transcript).toBe("keep-transcript");
    expect(state.dialogueHistory).toHaveLength(1);
    expect(state.interactionState).toBe("Working");
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("active-task pre-check opens the confirmation modal without ending the session", async () => {
    responses["get_active_tasks_count"] = 2;
    responses["get_runtime_snapshot"] = { pipeline_state: "Idle" };
    await mountProvider();
    seedActiveSession();

    await act(async () => {
      await actions!.disengage();
    });

    expect(calls.some((c) => c.cmd === "end_session")).toBe(false);
    expect(screen.getByRole("alertdialog")).toBeTruthy();
    expect(screen.getByText("Disengage Session?")).toBeTruthy();
    expect(screen.getByText(/You have 2 background processes running/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Terminate & Disengage" })).toBeTruthy();
  });

  it("confirming the modal sends force=true and clears state only after Idle is confirmed", async () => {
    responses["get_active_tasks_count"] = 2;
    responses["end_session"] = null;
    responses["get_runtime_snapshot"] = { pipeline_state: "Idle" };
    await mountProvider();
    seedActiveSession();

    await act(async () => {
      await actions!.disengage();
    });
    expect(screen.getByRole("alertdialog")).toBeTruthy();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Terminate & Disengage" }));
      await vi.advanceTimersByTimeAsync(200);
    });

    expect(calls.filter((c) => c.cmd === "end_session")).toEqual([
      { cmd: "end_session", args: { force: true } },
    ]);
    const state = useSessionStore.getState();
    expect(state.interactionState).toBe("Idle");
    expect(state.activeSessionId).toBeNull();
    expect(state.transcript).toBe("");
    expect(state.dialogueHistory).toEqual([]);

    // AnimatePresence cannot finish its framer-motion exit in jsdom, so the node
    // stays mounted. The inline opacity=0 is the observable proof that the
    // production confirm handler set open=false; a modal left open stays at 1.
    const dialog = screen.queryByRole("alertdialog");
    expect(dialog?.getAttribute("style")).toMatch(/opacity:\s*0/);
  });
});
