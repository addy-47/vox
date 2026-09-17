import { useRef, useCallback, useState, useEffect } from "react";
import {
  onTranscriptPartial,
  onTranscriptFinal,
  onLlmToken,
} from "@/services/eventsService";
import type { TranscriptPayload, LlmTokenPayload } from "@/services/eventsService";

export function useTranscriptStream(
  onTurnComplete?: (turn: { id: number; user: string; assistant: string }) => void,
) {
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const [activeTurnId, setActiveTurnId] = useState<number | null>(null);

  const activeTurnIdRef = useRef<number | null>(null);
  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const partialThrottleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const tokenThrottleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onTurnCompleteRef = useRef(onTurnComplete);

  useEffect(() => {
    onTurnCompleteRef.current = onTurnComplete;
  }, [onTurnComplete]);

  const commitTurn = useCallback(() => {
    if (partialThrottleTimer.current) {
      clearTimeout(partialThrottleTimer.current);
      partialThrottleTimer.current = null;
    }
    if (tokenThrottleTimer.current) {
      clearTimeout(tokenThrottleTimer.current);
      tokenThrottleTimer.current = null;
    }

    const turnId = activeTurnIdRef.current;
    const user = activeUserTextRef.current.trim();
    const assistant = activeAiTextRef.current.trim();
    if ((user || assistant) && onTurnCompleteRef.current) {
      onTurnCompleteRef.current({
        id: turnId ?? Date.now(),
        user,
        assistant,
      });
    }

    activeTurnIdRef.current = null;
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setActiveTurnId(null);
    setTranscript("");
    setAssistantText("");
  }, []);

  const clearTranscript = useCallback(() => {
    if (partialThrottleTimer.current) {
      clearTimeout(partialThrottleTimer.current);
      partialThrottleTimer.current = null;
    }
    if (tokenThrottleTimer.current) {
      clearTimeout(tokenThrottleTimer.current);
      tokenThrottleTimer.current = null;
    }
    activeTurnIdRef.current = null;
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setActiveTurnId(null);
    setTranscript("");
    setAssistantText("");
  }, []);

  useEffect(() => {
    let cancelled = false;

    const cleanups = [
      onTranscriptPartial((payload: TranscriptPayload) => {
        if (cancelled) return;
        if (
          payload.turn_id !== undefined &&
          activeTurnIdRef.current !== null &&
          payload.turn_id !== activeTurnIdRef.current
        ) {
          commitTurn();
        }

        if (payload.turn_id !== undefined && payload.turn_id !== activeTurnIdRef.current) {
          activeTurnIdRef.current = payload.turn_id;
          setActiveTurnId(payload.turn_id);
          activeUserTextRef.current = payload.text;
          activeAiTextRef.current = "";
          setAssistantText("");
        } else {
          activeUserTextRef.current = payload.text;
        }

        if (!partialThrottleTimer.current) {
          partialThrottleTimer.current = setTimeout(() => {
            if (!cancelled) setTranscript(activeUserTextRef.current);
            partialThrottleTimer.current = null;
          }, 30);
        }
      }),
      onTranscriptFinal((payload: TranscriptPayload) => {
        if (cancelled) return;
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
          setActiveTurnId(payload.turn_id);
          activeAiTextRef.current = "";
          setAssistantText("");
        }
        activeUserTextRef.current = payload.text;
        setTranscript(payload.text);
      }),
      onLlmToken((payload: LlmTokenPayload) => {
        if (cancelled) return;
        if (
          payload.turn_id !== undefined &&
          activeTurnIdRef.current !== null &&
          payload.turn_id !== activeTurnIdRef.current
        ) {
          commitTurn();
        }
        if (payload.turn_id !== undefined && payload.turn_id !== activeTurnIdRef.current) {
          activeTurnIdRef.current = payload.turn_id;
          setActiveTurnId(payload.turn_id);
          activeAiTextRef.current = payload.token;
        } else {
          activeAiTextRef.current += payload.token;
        }

        if (!tokenThrottleTimer.current) {
          tokenThrottleTimer.current = setTimeout(() => {
            if (!cancelled) setAssistantText(activeAiTextRef.current);
            tokenThrottleTimer.current = null;
          }, 30);
        }
      }),
    ];

    return () => {
      cancelled = true;
      cleanups.forEach((fn) => fn());
      if (partialThrottleTimer.current) clearTimeout(partialThrottleTimer.current);
      if (tokenThrottleTimer.current) clearTimeout(tokenThrottleTimer.current);
    };
  }, [commitTurn]);

  const resetForNextTurn = commitTurn;

  return {
    transcript,
    assistantText,
    activeTurnId,
    activeTurnIdRef,
    activeUserTextRef,
    activeAiTextRef,
    commitTurn,
    resetForNextTurn,
    clearTranscript,
  };
}
