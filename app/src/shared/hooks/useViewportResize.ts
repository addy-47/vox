import { useState, useEffect } from "react";
import {
  installViewportResizeGate,
  subscribeViewportFrame,
  subscribeViewportGate,
  isViewportResizing,
  getViewportLayout,
  type ViewportLayout,
} from "@/layout/viewportResize";
import { isCompactWidth, isRealMobileDevice } from "@/layout/breakpoints";

export type { ViewportLayout };

/**
 * Reactive view of the viewport-resize gate. Returns whether a resize is in
 * flight and reactive layout modes (layout, isMobile, isCompact, isNarrow).
 */
export function useViewportResize() {
  const [isResizing, setIsResizing] = useState<boolean>(() => isViewportResizing());
  const [layout, setLayout] = useState<ViewportLayout>(() => getViewportLayout());
  const [isMobile, setIsMobile] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return isRealMobileDevice() || window.innerWidth < 768;
  });
  const [isCompact, setIsCompact] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return isCompactWidth() && !(isRealMobileDevice() || window.innerWidth < 768);
  });
  const [isNarrow, setIsNarrow] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return window.innerWidth < 480;
  });

  useEffect(() => {
    let isMounted = true;
    installViewportResizeGate();
    setIsResizing(isViewportResizing());
    setLayout(getViewportLayout());

    const updateBreakpoints = () => {
      if (!isMounted) return;
      const w = window.innerWidth;
      const mobile = isRealMobileDevice() || w < 768;
      setIsMobile(mobile);
      setIsCompact(w < 1024 && !mobile);
      setIsNarrow(w < 480);
      setLayout(w < 1024 ? "compact" : "wide");
    };
    updateBreakpoints();

    const unsubGate = subscribeViewportGate((active) => {
      if (isMounted) setIsResizing(active);
    });
    const unsubFrame = subscribeViewportFrame(() => {
      updateBreakpoints();
    });
    return () => {
      isMounted = false;
      unsubGate();
      unsubFrame();
    };
  }, []);

  return { isResizing, layout, isMobile, isCompact, isNarrow };
}
