import { useState, useEffect, useRef, useCallback } from 'react';

/**
 * Creates a smooth "typing" illusion by draining a character queue dynamically.
 * Overrides point 6.4: pop speed scales with queue size to empty in ~200ms.
 */
export const useStreamingRenderer = (targetText: string) => {
  const [displayText, setDisplayText] = useState("");
  const queue = useRef<string[]>([]);
  const animationFrame = useRef<number | null>(null);
  const lastTargetText = useRef("");

  // When targetText updates, push the diff to the queue
  useEffect(() => {
    const last = lastTargetText.current;
    const isAdditive = targetText.startsWith(last);
    
    if (isAdditive && targetText.length > last.length) {
      const diff = targetText.slice(last.length);
      queue.current.push(...diff.split(""));
    } else if (!isAdditive || targetText.length < last.length) {
      // If the text was reset, corrected, or updated non-additively, sync instantly
      setDisplayText(targetText);
      queue.current = [];
    }
    lastTargetText.current = targetText;
  }, [targetText]);

  const tick = useCallback(() => {
    if (queue.current.length > 0) {
      // Dynamic scaling: pop more chars if the queue is backing up
      // Empty in ~10 ticks (approx 160-200ms at 60fps)
      const charsToPop = Math.max(1, Math.ceil(queue.current.length / 10));
      const popped = queue.current.splice(0, charsToPop).join("");
      
      setDisplayText(prev => prev + popped);
      animationFrame.current = requestAnimationFrame(tick);
    } else {
      animationFrame.current = null;
    }
  }, []);

  useEffect(() => {
    if (queue.current.length > 0 && !animationFrame.current) {
      animationFrame.current = requestAnimationFrame(tick);
    }
  }, [targetText, tick]);

  useEffect(() => {
    return () => {
      if (animationFrame.current) cancelAnimationFrame(animationFrame.current);
    };
  }, []);

  return displayText;
};
