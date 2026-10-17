import { useState, useEffect, useRef, useCallback } from "react";

export type VisibilityState = "HIDDEN" | "APPEARING" | "ACTIVE" | "FADING";

export interface Interaction {
  id: number;
  committedText: string;
  partialText: string;
  lastUpdateTime: number;
}

/**
 * Manages the ephemeral visibility state machine for the HUD.
 * States: HIDDEN -> APPEARING -> ACTIVE -> FADING -> HIDDEN
 * Includes "Hover-Pause" logic to prevent disappearance while the user is interacting.
 */
export const useVisibility = () => {
  const [state, setState] = useState<VisibilityState>("HIDDEN");
  const [isHovered, setIsHovered] = useState(false);

  const fadeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const appearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isHoveredRef = useRef(false);
  const stateRef = useRef<VisibilityState>("HIDDEN");
  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const setIsHoveredWithRef = useCallback((hovered: boolean) => {
    isHoveredRef.current = hovered;
    setIsHovered(hovered);
  }, []);

  const clearTimers = useCallback(() => {
    if (fadeTimer.current) {
      clearTimeout(fadeTimer.current);
      fadeTimer.current = null;
    }
    if (appearTimer.current) {
      clearTimeout(appearTimer.current);
      appearTimer.current = null;
    }
  }, []);

  // Transition to ACTIVE (e.g. on first non-empty partial)
  const show = useCallback(() => {
    clearTimers();
    const cur = stateRef.current;
    if (cur === "HIDDEN" || cur === "FADING") {
      setState("APPEARING");
      appearTimer.current = setTimeout(() => {
        setState((s) => (s === "APPEARING" ? "ACTIVE" : s));
        appearTimer.current = null;
      }, 50);
    } else {
      setState("ACTIVE");
    }
  }, [clearTimers]);

  // Transition to FADING (triggered strictly by auto-sleep)
  const startFade = useCallback(() => {
    if (isHoveredRef.current) {
      setState("ACTIVE"); // Stay active if hovered
      return;
    }

    clearTimers();
    setState("FADING");

    fadeTimer.current = setTimeout(() => {
      setState("HIDDEN");
      fadeTimer.current = null;
    }, 500); // Hardcoded 500ms fade transition
  }, [clearTimers]);

  // Cancel any active fading (e.g. when system wakes up from speech)
  const cancelFade = useCallback(() => {
    clearTimers();
    setState((prev) => (prev === "FADING" ? "ACTIVE" : prev));
  }, [clearTimers]);

  const hideImmediately = useCallback(() => {
    clearTimers();
    setState("HIDDEN");
  }, [clearTimers]);

  // Handle Hover Overrides
  useEffect(() => {
    if (isHovered) {
      // If we enter the tray, freeze the state at ACTIVE
      clearTimers();
      if (state !== "HIDDEN") {
        setState("ACTIVE");
      }
    }
  }, [isHovered, state, clearTimers]);

  useEffect(() => {
    return () => clearTimers();
  }, [clearTimers]);

  return {
    state,
    isHovered,
    setIsHovered: setIsHoveredWithRef,
    show,
    startFade,
    cancelFade,
    hideImmediately,
  };
};

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
    reset,
  };
};
