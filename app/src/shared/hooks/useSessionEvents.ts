import { useEffect, useRef } from "react";
import {
  onStateChanged,
  onTranscriptPartial,
  onTranscriptFinal,
  onLlmToken,
  onSettingsUpdated,
  onTurnMetrics,
  type StateChangedPayload,
  type TranscriptPayload,
  type LlmTokenPayload,
  type TurnMetricsPayload,
} from "@/services/eventsService";
import type { InteractionState } from "@/services/eventsService";

const VALID_STATES = new Set<InteractionState>([
  "Idle", "Ready", "Listening", "Thinking", "Speaking", "Paused", "Error", "Sleeping",
]);

function isValidState(s: string): s is InteractionState {
  return VALID_STATES.has(s as InteractionState);
}

interface SessionEventHandlers {
  onStateChanged: (payload: StateChangedPayload) => void;
  onTranscriptPartial: (payload: TranscriptPayload) => void;
  onTranscriptFinal: (payload: TranscriptPayload) => void;
  onLlmToken: (payload: LlmTokenPayload) => void;
  onSettingsUpdated: () => void;
  onTurnMetrics?: (payload: TurnMetricsPayload) => void;
}

export function useSessionEvents(handlers: SessionEventHandlers): void {
  const mounted = useRef(true);
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    mounted.current = true;
    const cleanups: (() => void)[] = [];

    cleanups.push(
      onStateChanged((payload: StateChangedPayload) => {
        if (!mounted.current) return;
        if (payload.owner === "Dictation") return;
        if (!isValidState(payload.state)) {
          console.warn(`[SessionEvents] Invalid state received: ${payload.state}`);
          return;
        }
        handlersRef.current.onStateChanged(payload);
      }),
    );

    cleanups.push(
      onTranscriptPartial((payload: TranscriptPayload) => {
        if (!mounted.current) return;
        handlersRef.current.onTranscriptPartial(payload);
      }),
    );

    cleanups.push(
      onTranscriptFinal((payload: TranscriptPayload) => {
        if (!mounted.current) return;
        handlersRef.current.onTranscriptFinal(payload);
      }),
    );

    cleanups.push(
      onLlmToken((payload: LlmTokenPayload) => {
        if (!mounted.current) return;
        handlersRef.current.onLlmToken(payload);
      }),
    );

    cleanups.push(
      onSettingsUpdated(() => {
        if (!mounted.current) return;
        handlersRef.current.onSettingsUpdated();
      }),
    );

    cleanups.push(
      onTurnMetrics((payload: TurnMetricsPayload) => {
        if (!mounted.current) return;
        handlersRef.current.onTurnMetrics?.(payload);
      }),
    );

    return () => {
      mounted.current = false;
      cleanups.forEach((fn) => fn());
    };
  }, []);
}
