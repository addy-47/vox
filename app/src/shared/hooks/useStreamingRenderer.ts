import { useState, useEffect, useRef } from 'react';

/**
 * Creates an ultra-smooth, jitter-free typing animation by interpolating
 * the visible string length using requestAnimationFrame.
 */
export const useStreamingRenderer = (targetText: string) => {
  const [displayText, setDisplayText] = useState("");
  const currentLengthRef = useRef(0);
  const animationFrameRef = useRef<number | null>(null);
  const prevTextRef = useRef("");

  useEffect(() => {
    const prevText = prevTextRef.current;
    
    // If text was reset, shortened, or changed non-additively, sync instantly
    if (!targetText.startsWith(prevText) || targetText.length < prevText.length) {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
      currentLengthRef.current = targetText.length;
      setDisplayText(targetText);
      prevTextRef.current = targetText;
      return;
    }

    prevTextRef.current = targetText;

    // Start or continue the animation loop
    const tick = () => {
      const targetLen = targetText.length;
      const curLen = currentLengthRef.current;

      if (document.hidden) {
        currentLengthRef.current = targetLen;
        setDisplayText(targetText);
        animationFrameRef.current = null;
        return;
      }

      if (curLen < targetLen) {
        // Smooth ease-out length transition (exponential catch-up)
        const step = Math.max(1.2, (targetLen - curLen) * 0.22);
        const nextLen = Math.min(targetLen, curLen + step);
        currentLengthRef.current = nextLen;

        setDisplayText(targetText.slice(0, Math.floor(nextLen)));
        animationFrameRef.current = requestAnimationFrame(tick);
      } else {
        animationFrameRef.current = null;
      }
    };

    if (animationFrameRef.current === null && currentLengthRef.current < targetText.length) {
      animationFrameRef.current = requestAnimationFrame(tick);
    }
  }, [targetText]);

  useEffect(() => {
    return () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, []);

  return displayText;
};

