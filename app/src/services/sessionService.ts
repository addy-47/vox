import { invoke } from "@tauri-apps/api/core";
import { getRuntimeSnapshot } from "./pipelineService";
import type { InteractionState } from "@/services/eventsService";

export type VoxIpcError = {
  readonly error_type: string;
  readonly message: string;
};

function isVoxIpcError(err: unknown): err is VoxIpcError {
  return (
    typeof err === "object" &&
    err !== null &&
    "error_type" in err &&
    "message" in err
  );
}

export function isInvalidStateError(err: VoxIpcError): boolean {
  return err.error_type === "InvalidState";
}

export interface SessionOrchestrationResult {
  readonly success: boolean;
  readonly reason?: "invalid_state" | "timeout" | "error";
}

const VALID_STATES = new Set<InteractionState>([
  "Idle", "Ready", "Listening", "Thinking", "Speaking", "Paused", "Error", "Sleeping", "Working",
]);

export async function startSession(sessionId?: number | null): Promise<void> {
  await invoke("start_session", { sessionId: sessionId ?? null });
}

export async function endSession(): Promise<void> {
  await invoke("end_session");
}

export async function pauseSession(): Promise<void> {
  await invoke("pause_session");
}

export async function resumeSession(): Promise<void> {
  await invoke("resume_session");
}

export async function pttStart(): Promise<void> {
  await invoke("ptt_start");
}

export async function pttStop(): Promise<void> {
  await invoke("ptt_stop");
}

export async function pttCancel(): Promise<void> {
  await invoke("ptt_cancel");
}

export async function engageSession(
  notifyState: (state: InteractionState) => void,
  timeoutMs: number = 8000,
): Promise<SessionOrchestrationResult> {
  try {
    await startSession();
  } catch (err: unknown) {
    if (isVoxIpcError(err) && isInvalidStateError(err)) {
      const synced = await resyncFromSnapshot();
      return { success: synced.success, reason: synced.success ? undefined : "invalid_state" };
    }
    return { success: false, reason: "error" };
  }

  try {
    const reached = await waitForState("Ready", notifyState, timeoutMs);
    if (!reached) {
      const synced = await resyncFromSnapshot();
      return { success: synced.success, reason: synced.success ? undefined : "timeout" };
    }
    return { success: true };
  } catch {
    const synced = await resyncFromSnapshot();
    return { success: synced.success, reason: synced.success ? undefined : "timeout" };
  }
}

export async function disengageSession(
  notifyState: (state: InteractionState) => void,
  timeoutMs: number = 8000,
): Promise<SessionOrchestrationResult> {
  try {
    await endSession();
  } catch (err: unknown) {
    if (isVoxIpcError(err) && isInvalidStateError(err)) {
      const synced = await resyncFromSnapshot();
      return { success: synced.success, reason: synced.success ? undefined : "invalid_state" };
    }
    return { success: false, reason: "error" };
  }

  try {
    const reached = await waitForState("Idle", notifyState, timeoutMs);
    if (!reached) {
      const synced = await resyncFromSnapshot();
      return { success: synced.success, reason: synced.success ? undefined : "timeout" };
    }
    return { success: true };
  } catch {
    const synced = await resyncFromSnapshot();
    return { success: synced.success, reason: synced.success ? undefined : "timeout" };
  }
}

async function waitForState(
  target: InteractionState,
  notifyState: (state: InteractionState) => void,
  timeoutMs: number,
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  // The live `state_changed` listener (subscribed synchronously at mount)
  // is the primary path. This poll loop is the safety net: it reconciles
  // from the snapshot so a missed event still converges.
  for (;;) {
    try {
      const snap = await getRuntimeSnapshot();
      const state = snap?.pipeline_state as InteractionState | undefined;
      if (state && VALID_STATES.has(state)) {
        notifyState(state);
        if (state === target) return true;
      }
    } catch {
      // Best-effort; keep polling until deadline.
    }
    if (Date.now() >= deadline) return false;
    await new Promise((r) => setTimeout(r, 500));
  }
}

async function resyncFromSnapshot(): Promise<{ success: boolean }> {
  try {
    const snap = await getRuntimeSnapshot();
    if (snap?.pipeline_state && VALID_STATES.has(snap.pipeline_state as InteractionState)) {
      return { success: true };
    }
    return { success: false };
  } catch {
    return { success: false };
  }
}
