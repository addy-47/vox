import { useState, useEffect, useRef, useCallback } from 'react';

export type VisibilityState = 'HIDDEN' | 'APPEARING' | 'ACTIVE' | 'FADING';

/**
 * Manages the ephemeral visibility state machine for the HUD.
 * States: HIDDEN -> APPEARING -> ACTIVE -> FADING -> HIDDEN
 * Includes "Hover-Pause" logic to prevent disappearance while the user is interacting.
 */
export const useVisibility = () => {
  const [state, setState] = useState<VisibilityState>('HIDDEN');
  const [isHovered, setIsHovered] = useState(false);
  
  const fadeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const appearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isHoveredRef = useRef(false);
  const stateRef = useRef<VisibilityState>('HIDDEN');
  stateRef.current = state;

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
    if (cur === 'HIDDEN' || cur === 'FADING') {
      setState('APPEARING');
      appearTimer.current = setTimeout(() => {
        setState(s => (s === 'APPEARING' ? 'ACTIVE' : s));
        appearTimer.current = null;
      }, 50);
    } else {
      setState('ACTIVE');
    }
  }, [clearTimers]);

  // Transition to FADING (triggered strictly by auto-sleep)
  const startFade = useCallback(() => {
    if (isHoveredRef.current) {
      setState('ACTIVE'); // Stay active if hovered
      return;
    }
    
    clearTimers();
    setState('FADING');
    
    fadeTimer.current = setTimeout(() => {
      setState('HIDDEN');
      fadeTimer.current = null;
    }, 500); // Hardcoded 500ms fade transition
  }, [clearTimers]);

  // Cancel any active fading (e.g. when system wakes up from speech)
  const cancelFade = useCallback(() => {
    clearTimers();
    setState(prev => (prev === 'FADING' ? 'ACTIVE' : prev));
  }, [clearTimers]);

  const hideImmediately = useCallback(() => {
    clearTimers();
    setState('HIDDEN');
  }, [clearTimers]);

  // Handle Hover Overrides
  useEffect(() => {
    if (isHovered) {
      // If we enter the tray, freeze the state at ACTIVE
      clearTimers();
      if (state !== 'HIDDEN') {
        setState('ACTIVE');
      }
    } else if (state === 'ACTIVE' || state === 'FADING') {
      // If we exit and are sleeping, let it fade out
      // otherwise stay active
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
    hideImmediately
  };
};
