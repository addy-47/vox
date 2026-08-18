import { useState, useRef, useCallback } from 'react';

export interface Interaction {
  id: number;
  committedText: string;
  partialText: string;
  lastUpdateTime: number;
}

/**
 * Manages logical interaction sessions by keeping visual history alive.
 * Decouples VAD start/end from UX-level "Interaction Sessions".
 */
export const useInteraction = () => {
  const [interactionId, setInteractionId] = useState(0);
  const [committedText, setCommittedText] = useState("");
  const [partialText, setPartialText] = useState("");
  
  const lastSpeechEndTime = useRef<number>(0);
  const currentIdRef = useRef<number>(0);

  const startNewInteraction = useCallback(() => {
    // In the persistent session model, starting a new interaction is handled
    // strictly by manual clear or waking up after auto-sleep. We keep the ID stable.
    if (currentIdRef.current === 0) {
      currentIdRef.current = 1;
      setInteractionId(currentIdRef.current);
      setCommittedText("");
      setPartialText("");
      console.log(`[Interaction] >>> Session Initiated: ${currentIdRef.current}`);
    }
  }, []);

  const endSpeechSegment = useCallback(() => {
    lastSpeechEndTime.current = Date.now();
  }, []);

  const updatePartial = useCallback((text: string) => {
    setPartialText(text);
  }, []);

  const commitFinal = useCallback((text: string) => {
    if (!text) return;
    setCommittedText((prev) => {
      const separator = prev ? "\n" : "";
      const full = prev + separator + text;
      // Cap committed text to recent 4,000 characters to prevent unbounded RAM growth
      return full.length > 4000 ? full.slice(full.length - 4000) : full;
    });
    setPartialText("");
  }, []);

  const reset = useCallback(() => {
    setCommittedText("");
    setPartialText("");
    currentIdRef.current = 0;
    setInteractionId(0);
  }, []);

  return {
    interactionId,
    committedText,
    partialText,
    startNewInteraction,
    endSpeechSegment,
    updatePartial,
    commitFinal,
    reset
  };
};
