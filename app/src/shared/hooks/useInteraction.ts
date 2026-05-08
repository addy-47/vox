import { useState, useRef, useCallback } from 'react';

export interface Interaction {
  id: number;
  committedText: string;
  partialText: string;
  lastUpdateTime: number;
}

const CONTINUITY_WINDOW = 1200; // ms

/**
 * Manages logical interaction sessions by grouping noisy VAD segments.
 * Decouples VAD start/end from UX-level "Interaction Sessions".
 */
export const useInteraction = () => {
  const [interactionId, setInteractionId] = useState(0);
  const [committedText, setCommittedText] = useState("");
  const [partialText, setPartialText] = useState("");
  
  const lastSpeechEndTime = useRef<number>(0);
  const currentIdRef = useRef<number>(0);

  const startNewInteraction = useCallback(() => {
    const now = Date.now();
    const diff = now - lastSpeechEndTime.current;

    // Continuity Rule: If the gap is small, keep the same interaction
    if (diff > CONTINUITY_WINDOW || currentIdRef.current === 0) {
      currentIdRef.current += 1;
      setInteractionId(currentIdRef.current);
      setCommittedText("");
      setPartialText("");
      console.log(`[Interaction] >>> New Session Started: ${currentIdRef.current}`);
    } else {
      console.log(`[Interaction] Merging with existing session: ${currentIdRef.current}`);
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
    setCommittedText(prev => {
      const separator = prev ? " " : "";
      return prev + separator + text;
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
