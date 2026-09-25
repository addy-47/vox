import { invoke } from "@tauri-apps/api/core";
import { getRuntimeSnapshot } from "./monitoringService";
import type { InteractionState } from "@/services/eventsService";
import type { SessionRow, TurnRow } from "./historyService";
import { useSessionStore } from "@/store/sessionStore";

export type InteractionOwner = "Assistant" | "Dictation";

export type VoxIpcError = {
  readonly error_type: string;
  readonly message: string;
};

export function isVoxIpcError(err: unknown): err is VoxIpcError {
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

export interface ContinueSessionResult {
  session: SessionRow;
  turns: TurnRow[];
}

// ── Engine Lifecycle ────────────────────────────────────────────────────────

export function stopEngine(): Promise<void> {
  return invoke("stop_engine");
}

export function launchEngine(): Promise<void> {
  return invoke("launch_engine");
}

export function restartEngine(): Promise<void> {
  return invoke("restart_engine");
}

// ── Session Lifecycle (ipc/pipeline.rs) ──────────────────────────────────────

export function startSession(sessionId?: number | null): Promise<void> {
  return invoke("start_session", { sessionId: sessionId ?? null });
}

export function endSession(): Promise<void> {
  return invoke("end_session");
}

export function pauseSession(): Promise<void> {
  return invoke("pause_session");
}

export function resumeSession(): Promise<void> {
  return invoke("resume_session");
}

export async function createSession(
  projectId?: string,
  notifyState?: (state: InteractionState) => void,
): Promise<SessionRow | null> {
  try {
    const snap = await getRuntimeSnapshot();
    if (snap && snap.pipeline_state !== "Idle") {
      if (notifyState) {
        await disengageSession(notifyState);
      } else {
        await endSession();
      }
    }
  } catch {
    // Best-effort cleanup prior to creating new session
  }
  return invoke("create_session", { projectId: projectId ?? null });
}

export async function continueSession(
  sessionId: number,
  notifyState?: (state: InteractionState) => void,
): Promise<ContinueSessionResult> {
  // 1. If currently active, cleanly end the old session immediately
  try {
    const snap = await getRuntimeSnapshot();
    if (snap && snap.pipeline_state !== "Idle") {
      await endSession();
      if (notifyState) {
        notifyState("Idle");
      }
    }
  } catch {
    // Best-effort cleanup prior to continuing session
  }

  // 2. Fetch session data & set conversation_id in backend (<5ms SQLite query)
  const result = await invoke<ContinueSessionResult>("continue_session", { sessionId });

  // 3. Auto-engage pipeline with the new sessionId in background so caller receives
  // historical turns immediately without waiting for CPAL audio engine boot polling.
  (async () => {
    try {
      if (notifyState) {
        await engageSession(notifyState, 8000, sessionId);
      } else {
        await startSession(sessionId);
      }
    } catch (e) {
      console.warn("[continueSession] Background auto-engagement notice:", e);
    }
  })();

  return result;
}

// ── Orchestration Helpers ───────────────────────────────────────────────────

export async function engageSession(
  notifyState: (state: InteractionState) => void,
  timeoutMs: number = 8000,
  sessionId?: number | null,
): Promise<SessionOrchestrationResult> {
  try {
    await startSession(sessionId);
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
  for (;;) {
    // 1. Fast-path: check if push event already reached the target state
    if (useSessionStore.getState().interactionState === target) {
      return true;
    }

    try {
      const snap = await getRuntimeSnapshot();
      const state = snap?.pipeline_state as InteractionState | undefined;
      // Only notify when target is actually confirmed — never push stale intermediate states back to the store
      if (state === target) {
        notifyState(target);
        return true;
      }
    } catch {
      // Best-effort; keep polling until deadline.
    }
    if (Date.now() >= deadline) return false;
    await new Promise((r) => setTimeout(r, 100));
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

// ── Input & Controls ────────────────────────────────────────────────────────

export function submitTextInput(query: string): Promise<void> {
  return invoke("submit_text_input", { query });
}

export function pttStart(): Promise<void> {
  return invoke("ptt_start");
}

export function pttStop(): Promise<void> {
  return invoke("ptt_stop");
}

export function pttCancel(): Promise<void> {
  return invoke("ptt_cancel");
}

export function setPlaybackMuted(muted: boolean): Promise<void> {
  return invoke("set_playback_muted", { muted });
}

export function setMicMuted(muted: boolean): Promise<void> {
  return invoke("set_mic_muted", { muted });
}

export function setSessionPrivateMode(enabled: boolean): Promise<void> {
  return invoke("set_session_private_mode", { enabled });
}

// ── Re-exports for Backward Compatibility ────────────────────────────────────

export { getRuntimeSnapshot, type RuntimeSnapshot, type LocalSnapshot } from "./monitoringService";
export {
  listVoices,
  renameVoice,
  addVoiceFromFile,
  addVoiceFromRecording,
  deleteVoice,
  startBackendRecording,
  stopBackendRecording,
  type VoiceEntryDto,
  type EdgeTtsVoiceDto,
} from "./voiceService";
export { setupRemoteServer, type RemoteServerConfig } from "./settingsService";
