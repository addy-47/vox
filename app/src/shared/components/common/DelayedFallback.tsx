import React, { useState, useEffect } from "react";

export interface DelayedFallbackProps {
  /** Delay in milliseconds before displaying children. Default: 150ms. */
  delayMs?: number;
  children: React.ReactNode;
}

/**
 * Floor guard wrapper for Suspense fallbacks.
 * Prevents flashing loading indicators when async resolution completes within delayMs.
 */
export const DelayedFallback: React.FC<DelayedFallbackProps> = ({
  delayMs = 150,
  children,
}) => {
  const [show, setShow] = useState(() => delayMs <= 0);

  useEffect(() => {
    if (delayMs <= 0) return;
    const timer = setTimeout(() => setShow(true), delayMs);
    return () => clearTimeout(timer);
  }, [delayMs]);

  if (!show) return null;
  return <>{children}</>;
};
