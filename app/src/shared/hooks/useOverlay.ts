import { useEffect, useRef } from "react";
import { registerOverlay } from "@/shared/lib/overlayStack";

interface UseOverlayOptions {
  /** Called when the overlay is dismissed by the stack (Escape / outside-click). */
  onClose: () => void;
  /**
   * Whether the overlay is currently open. When false the overlay is not
   * registered on the stack. Defaults to true (register on mount).
   */
  active?: boolean;
  /** Ref to the overlay's root element, used for outside-click dismissal. */
  ref?: React.RefObject<HTMLElement | null>;
  /**
   * If true, a pointerdown outside `ref` dismisses the overlay. For overlays
   * that render their own backdrop, leave false — the backdrop handles it.
   */
  dismissOnOutside?: boolean;
}

/**
 * Registers an overlay with the global overlay stack while it is open.
 * Escape and (optionally) outside-click dismissal are handled centrally by
 * the stack — per-surface listeners should be removed in favor of this hook.
 */
export function useOverlay({ onClose, active = true, ref, dismissOnOutside = false }: UseOverlayOptions): void {
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const refRef = useRef(ref);
  refRef.current = ref;
  const dismissRef = useRef(dismissOnOutside);
  dismissRef.current = dismissOnOutside;

  useEffect(() => {
    if (!active) return;
    return registerOverlay({
      onClose: () => onCloseRef.current(),
      getEl: () => refRef.current?.current ?? null,
      dismissOnOutside: dismissRef.current,
    });
  }, [active]);
}