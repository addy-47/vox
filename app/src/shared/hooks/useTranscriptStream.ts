import { useRef, useCallback, useState, useEffect } from "react";
import {
  onTranscriptPartial,
  onTranscriptFinal,
  onLlmToken,
} from "@/services/eventsService";
import type { TranscriptPayload, LlmTokenPayload } from "@/services/eventsService";

export function useTranscriptStream() {
  const [transcript, setTranscript] = useState("");
  const [assistantText, setAssistantText] = useState("");
  const activeUserTextRef = useRef("");
  const activeAiTextRef = useRef("");
  const partialThrottleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const tokenThrottleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let cancelled = false;

    const cleanups = [
      onTranscriptPartial((payload: TranscriptPayload) => {
        if (cancelled) return;
        activeUserTextRef.current = payload.text;
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
        activeUserTextRef.current = payload.text;
        setTranscript(payload.text);
      }),
      onLlmToken((payload: LlmTokenPayload) => {
        if (cancelled) return;
        activeAiTextRef.current += payload.token;
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
  }, []);

  const clearTranscript = useCallback(() => {
    activeUserTextRef.current = "";
    activeAiTextRef.current = "";
    setTranscript("");
    setAssistantText("");
  }, []);

  return { transcript, assistantText, clearTranscript };
}
