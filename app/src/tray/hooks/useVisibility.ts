import { useState, useEffect, useRef, useCallback } from 'react';

export type VisibilityState = 'HIDDEN' | 'APPEARING' | 'ACTIVE' | 'HOLD' | 'FADING';

interface VisibilityConfig {
  holdDuration?: number;
  fadeDuration?: number;
}

/**
 * Manages the ephemeral visibility state machine for the HUD.
 * States: HIDDEN -> APPEARING -> ACTIVE -> HOLD -> FADING -> HIDDEN
 * Includes "Hover-Pause" logic to prevent disappearance while the user is interacting.
 */
export const useVisibility = (config: VisibilityConfig = {}) => {
  const { holdDuration = 3000, fadeDuration = 2000 } = config;
  
  const [state, setState] = useState<VisibilityState>('HIDDEN');
  const [isHovered, setIsHovered] = useState(false);
  
  const holdTimer = useRef<NodeJS.Timeout | null>(null);
  const fadeTimer = useRef<NodeJS.Timeout | null>(null);

  const clearTimers = useCallback(() => {
    if (holdTimer.current) clearTimeout(holdTimer.current);
    if (fadeTimer.current) clearTimeout(fadeTimer.current);
    holdTimer.current = null;
    fadeTimer.current = null;
  }, []);

  // Transition to ACTIVE (e.g. on speech_start)
  const show = useCallback(() => {
    clearTimers();
    if (state === 'HIDDEN' || state === 'FADING') {
      setState('APPEARING');
      // Small delay to allow transition
      setTimeout(() => setState('ACTIVE'), 50);
    } else {
      setState('ACTIVE');
    }
  }, [state, clearTimers]);

  // Transition to HOLD (e.g. on speech_end)
  const startHold = useCallback(() => {
    if (isHovered) {
      setState('ACTIVE'); // Stay active if hovered
      return;
    }
    
    clearTimers();
    setState('HOLD');
    
    holdTimer.current = setTimeout(() => {
      setState('FADING');
      fadeTimer.current = setTimeout(() => {
        setState('HIDDEN');
      }, fadeDuration);
    }, holdDuration);
  }, [isHovered, holdDuration, fadeDuration, clearTimers]);

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
    } else if (state === 'ACTIVE' || state === 'HOLD') {
      // If we exit, restart the hold timer
      startHold();
    }
  }, [isHovered]);

  return {
    state,
    isHovered,
    setIsHovered,
    show,
    startHold,
    hideImmediately
  };
};
