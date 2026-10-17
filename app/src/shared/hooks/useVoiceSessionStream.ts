import { useCallback, useEffect, useRef, useState } from "react";
import {
  onLlmToken,
  onSettingsUpdated,
  onStateChanged,
  onTranscriptFinal,
  onTranscriptPartial,
  onTurnMetrics,
  VALID_INTERACTION_STATES,
  type InteractionState,
  type LlmTokenPayload,
  type StateChangedPayload,
  type TranscriptPayload,
  type TurnMetricsPayload,
} from "@/services/eventsService";
import { isDictationOwner } from "@/services/pipelineService";
import { getSettings } from "@/services/settingsService";
import { getRuntimeSnapshot } from "@/services/monitoringService";
import { getTurns } from "@/services/persistenceService";

/** A completed dialogue turn flushed into the session history. */
export interface VoiceSessionTurn {
  user: string;
  assistant: string;
  id: number;
}

/** Initial-state snapshot assembled from settings, runtime snapshot, and persisted turns. */
export interface VoiceSessionHydration {
  interactionMode: string | null;
  pipelineMode: "modular" | "realtime" | null;
  cpuWarning: { governor: string } | null;
  activeSessionId: number | null;
  dialogueHistory: VoiceSessionTurn[];
  turnIdCounter: number;
}

export interface VoiceSessionStreamHandlers {
  onStateChanged: (payload: StateChangedPayload) => void;
  onSettingsUpdated: () => void;
  onTurnMetrics?: (payload: TurnMetricsPayload) => void;
  /** Invoked when an in-flight streaming turn is flushed into history. */
  onTurnComplete?: (turn: VoiceSessionTurn) => void;
  /** Invoked once after the first (post-subscription) hydration pass completes. */
  onHydrated: (result: VoiceSessionHydration) => void;
}

/**
 * Single lifecycle hook owning the voice session event stream. Consolidates the
 * former useSessionEvents + useSessionHydration + useTranscriptStream trio:
 * one subscription set (state_changed, transcript partial/final, llm tokens,
 * settings-updated, turn metrics), streaming transcript state with 30ms
 * commit throttling, and first-mount state hydration.
 */
export function useVoiceSessionStream(handlers: VoiceSessionStreamHandlers): {
  transcript: string;
  assistantText: string;
  commitTurn: () => void;
  clearTranscript: () => void;
} {
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");

  const mounted = useRef(true);
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  const activeTurnIdRef = useRef<number | null>(null);
  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const partialThrottleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const tokenThrottleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimers = useCallback(() => {
    if (partialThrottleTimer.current) {
      clearTimeout(partialThrottleTimer.current);
      partialThrottleTimer.current = null;
    }
    if (tokenThrottleTimer.current) {
      clearTimeout(tokenThrottleTimer.current);
      tokenThrottleTimer.current = null;
    }
  }, []);

  const commitTurn = useCallback(() => {
    clearTimers();

    const turnId = activeTurnIdRef.current;
    const user = activeUserTextRef.current.trim();
    const assistant = activeAiTextRef.current.trim();
    console.info("[Stream] commitTurn", { turnId, userLen: user.length, assistantLen: assistant.length });
    if ((user || assistant) && handlersRef.current.onTurnComplete) {
      handlersRef.current.onTurnComplete({
        id: turnId ?? Date.now(),
        user,
        assistant,
      });
    }

    activeTurnIdRef.current = null;
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setTranscript("");
    setAssistantText("");
  }, [clearTimers]);

  const clearTranscript = useCallback(() => {
    clearTimers();
    activeTurnIdRef.current = null;
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setTranscript("");
    setAssistantText("");
  }, [clearTimers]);

  // Subscriptions register synchronously on mount, before any await.
  // Declared before the hydration effect below so listeners exist first.
  useEffect(() => {
    mounted.current = true;

    const cleanups = [
      onStateChanged((payload: StateChangedPayload) => {
        if (!mounted.current) return;
        if (isDictationOwner(payload.owner)) return;
        if (!VALID_INTERACTION_STATES.has(payload.state as InteractionState)) {
          console.warn(`[VoiceSessionStream] Invalid state received: ${payload.state}`);
          return;
        }
        handlersRef.current.onStateChanged(payload);
      }),
      onTranscriptPartial((payload: TranscriptPayload) => {
        if (!mounted.current) return;
        if (isDictationOwner(payload.owner)) return;
        if (
          payload.turn_id !== undefined &&
          activeTurnIdRef.current !== null &&
          payload.turn_id !== activeTurnIdRef.current
        ) {
          commitTurn();
        }

        if (payload.turn_id !== undefined && payload.turn_id !== activeTurnIdRef.current) {
          activeTurnIdRef.current = payload.turn_id;
          activeUserTextRef.current = payload.text;
          activeAiTextRef.current = "";
          setAssistantText("");
        } else {
          activeUserTextRef.current = payload.text;
        }

        if (!partialThrottleTimer.current) {
          partialThrottleTimer.current = setTimeout(() => {
            if (!mounted.current) return;
            setTranscript(activeUserTextRef.current);
            partialThrottleTimer.current = null;
          }, 30);
        }
      }),
      onTranscriptFinal((payload: TranscriptPayload) => {
        if (!mounted.current) return;
        if (isDictationOwner(payload.owner)) return;
        if (partialThrottleTimer.current) {
          clearTimeout(partialThrottleTimer.current);
          partialThrottleTimer.current = null;
        }
        if (
          payload.turn_id !== undefined &&
          activeTurnIdRef.current !== null &&
          payload.turn_id !== activeTurnIdRef.current
        ) {
          commitTurn();
        }
        if (payload.turn_id !== undefined && payload.turn_id !== activeTurnIdRef.current) {
          activeTurnIdRef.current = payload.turn_id;
          activeAiTextRef.current = "";
          setAssistantText("");
        }
        activeUserTextRef.current = payload.text;
        setTranscript(payload.text);
      }),
      onLlmToken((payload: LlmTokenPayload) => {
        if (!mounted.current) return;
        if (
          payload.turn_id !== undefined &&
          activeTurnIdRef.current !== null &&
          payload.turn_id !== activeTurnIdRef.current
        ) {
          commitTurn();
        }
        if (payload.turn_id !== undefined && payload.turn_id !== activeTurnIdRef.current) {
          activeTurnIdRef.current = payload.turn_id;
          activeAiTextRef.current = payload.token;
          console.info("[Stream] llm_token: new turn", payload.turn_id, "first token chars", payload.token.length);
        } else {
          activeAiTextRef.current += payload.token;
        }

        if (!tokenThrottleTimer.current) {
          tokenThrottleTimer.current = setTimeout(() => {
            if (!mounted.current) return;
            setAssistantText(activeAiTextRef.current);
            tokenThrottleTimer.current = null;
          }, 30);
        }
      }),
      onSettingsUpdated(() => {
        if (!mounted.current) return;
        handlersRef.current.onSettingsUpdated();
      }),
      onTurnMetrics((payload: TurnMetricsPayload) => {
        if (!mounted.current) return;
        handlersRef.current.onTurnMetrics?.(payload);
      }),
    ];

    return () => {
      mounted.current = false;
      cleanups.forEach((fn) => fn());
      clearTimers();
    };
  }, [commitTurn, clearTimers]);

  // Hydration runs after the subscription above is established.
  useEffect(() => {
    let cancelled = false;

    const hydrate = async () => {
      try {
        const res = await getSettings();
        const interactionMode = res.settings?.interaction?.mode ?? null;
        const pipelineMode = res.settings?.interaction?.pipeline_mode
          ? (res.settings.interaction.pipeline_mode.toLowerCase() as "modular" | "realtime")
          : null;

        const snap = await getRuntimeSnapshot();
        if (cancelled) return;

        const cpuWarning = snap?.cpu_governor && !snap.cpu_governor_optimal
          ? { governor: snap.cpu_governor }
          : null;

        let activeSessionId: number | null = null;
        let dialogueHistory: VoiceSessionTurn[] = [];
        let turnIdCounter = 0;

        if (snap?.conversation_id && snap.conversation_id !== 0) {
          activeSessionId = snap.conversation_id;
          const turns = await getTurns(snap.conversation_id);
          if (cancelled) return;
          const history = turns.slice(-100).map((t) => ({
            user: t.user_text,
            assistant: t.assistant_text,
            id: t.turn_id,
          }));
          dialogueHistory = history;
          if (history.length > 0) {
            turnIdCounter = Math.max(...history.map((h) => h.id));
          }
        }

        if (!cancelled) {
          handlersRef.current.onHydrated({ interactionMode, pipelineMode, cpuWarning, activeSessionId, dialogueHistory, turnIdCounter });
        }
      } catch (e) {
        if (!cancelled) {
          console.warn("[VoiceSessionStream] Failed to sync initial state:", e);
        }
      }
    };

    const id = setTimeout(hydrate, 0);
    return () => { cancelled = true; clearTimeout(id); };
  }, []);

  return {
    transcript,
    assistantText,
    commitTurn,
    clearTranscript,
  };
}
