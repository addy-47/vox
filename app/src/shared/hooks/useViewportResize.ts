import { useState, useEffect } from "react";
import {
  installViewportResizeGate,
  subscribeViewportFrame,
  subscribeViewportGate,
  isViewportResizing,
  getViewportLayout,
  type ViewportLayout,
} from "@/layout/viewportResize";

export type { ViewportLayout };

/**
 * Reactive view of the viewport-resize gate. Returns whether a resize is in
 * flight (animations/blur suspended via `data-viewport-resize`) and the
 * current compact/wide layout mode. Fine-grained selectors: destructure only
 * what the component needs.
 */
export function useViewportResize() {
  const [isResizing, setIsResizing] = useState<boolean>(() => isViewportResizing());
  const [layout, setLayout] = useState<ViewportLayout>(() => getViewportLayout());

  useEffect(() => {
    let isMounted = true;
    installViewportResizeGate();
    setIsResizing(isViewportResizing());
    setLayout(getViewportLayout());

    const unsubGate = subscribeViewportGate((active) => {
      if (isMounted) setIsResizing(active);
    });
    const unsubFrame = subscribeViewportFrame(() => {
      if (isMounted) setLayout(getViewportLayout());
    });
    return () => {
      isMounted = false;
      unsubGate();
      unsubFrame();
    };
  }, []);

  return { isResizing, layout };
}
