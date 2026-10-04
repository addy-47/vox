import { memo, useEffect, useRef, useCallback, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { LAYOUT_COPY } from "@/data/layoutCopy";

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  subtitle?: React.ReactNode;
  icon?: React.ReactNode;
  /** Actions rendered on the right side of the header, before the close button. */
  headerActions?: React.ReactNode;
  /**
   * "page" — the modal layers under app chrome (default z-30). "global" —
   * layers above app chrome (default z-60), used for app-level surfaces.
   */
  position?: "page" | "global";
  /** Renders a dimmed blurred backdrop; clicking it closes the modal. */
  backdrop?: boolean;
  zIndex?: number;
  /** Fixed footer rendered below the scrollable body (shrink-0). */
  footer?: React.ReactNode;
  ariaLabel?: string;
  /** Panel sizing classes. Defaults to a responsive centered dialog. */
  className?: string;
  bodyClassName?: string;
  children: React.ReactNode;
}

/**
 * The single centered dialog used across Vox for Tier 2b surfaces (compact
 * Personal Memory). Provides the unified overlay contract: dimmed backdrop
 * with click-outside close, Escape (via the global overlay stack), focus
 * trap + restore, scale-fade motion.
 *
 * Sibling of `Drawer` (the bottom sheet) with the same header/footer/body
 * shape and the same `position` semantics — pick Drawer for edge-anchored
 * sheets, Modal for centered dialogs. A drag-resize handle is deliberately
 * absent: it is meaningless on a centered dialog.
 */
export const Modal = memo(
  ({
    open,
    onClose,
    title,
    subtitle,
    icon,
    headerActions,
    position = "page",
    backdrop = true,
    zIndex,
    footer,
    ariaLabel = LAYOUT_COPY.modal.defaultAria,
    className,
    bodyClassName,
    children,
  }: ModalProps) => {
    const panelRef = useRef<HTMLDivElement>(null);

    // Register with the global overlay stack so Escape closes this modal.
    useOverlay({
      onClose,
      active: open,
      ref: panelRef,
      dismissOnOutside: false, // backdrop handles outside clicks
    });

    // Focus the panel on open; restore focus to the trigger on close.
    const previouslyFocusedRef = useRef<HTMLElement | null>(null);
    useEffect(() => {
      if (!open) return;
      previouslyFocusedRef.current = document.activeElement as HTMLElement | null;
      panelRef.current?.focus();
      return () => {
        const prev = previouslyFocusedRef.current;
        previouslyFocusedRef.current = null;
        // Restore focus only if the trigger is still in the document and was
        // genuinely focusable. Otherwise blur so focus doesn't get stranded
        // inside the unmounting panel (previously it landed on <body>).
        if (prev && document.contains(prev)) {
          const focusable =
            prev.tabIndex >= 0 || /^(BUTTON|INPUT|SELECT|TEXTAREA|A)$/.test(prev.tagName);
          if (focusable) {
            prev.focus({ preventScroll: true });
            return;
          }
        }
        (document.activeElement as HTMLElement | null)?.blur?.();
      };
    }, [open]);

    // Focus trap: Tab cycles inside the modal panel; focus never escapes to
    // the page behind while the modal is open (aria-modal="true").
    const handlePanelKeyDown = useCallback((e: React.KeyboardEvent) => {
      if (e.key !== "Tab" || !panelRef.current) return;
      const items = Array.from(
        panelRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
        )
      ).filter((el) => el.offsetWidth > 0 || el.offsetHeight > 0);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey) {
        if (active === first || !panelRef.current.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last || !panelRef.current.contains(active)) {
        e.preventDefault();
        first.focus();
      }
    }, []);

    const effectiveZ = zIndex ?? (position === "global" ? 60 : 30);

    const modalNode = (
      <AnimatePresence>
        {open && (
          <div
            className="fixed inset-0 z-[var(--modal-z)] pointer-events-none flex items-center justify-center p-4 sm:p-6"
            style={{ ["--modal-z"]: effectiveZ } as CSSProperties}
          >
            {backdrop && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2 }}
                onClick={onClose}
                className="absolute inset-0 bg-black/50 backdrop-blur-[2px] pointer-events-auto cursor-default"
              />
            )}

            <motion.div
              ref={panelRef}
              role="dialog"
              aria-modal="true"
              aria-label={ariaLabel}
              tabIndex={-1}
              initial={{ opacity: 0, scale: 0.96, y: 8 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.96, y: 8 }}
              transition={{ duration: 0.24, ease: [0.16, 1, 0.3, 1] }}
              onKeyDown={handlePanelKeyDown}
              className={cn(
                "relative flex flex-col rounded-3xl overflow-hidden glass-card border border-[rgba(var(--accent),0.12)] [text-shadow:none] outline-none pointer-events-auto transform-gpu will-change-transform contain-paint",
                // Responsive default: nearly full-bleed on small screens,
                // capped on desktop. Consumers override via className.
                "w-[min(1120px,92vw)] h-[min(760px,88vh)]",
                className
              )}
              onClick={(e) => e.stopPropagation()}
            >
              {(title || icon || headerActions) && (
                <div className="flex items-center justify-between px-6 pt-4 pb-3 border-b border-[rgba(var(--accent),0.08)] shrink-0">
                  <div className="flex items-center gap-3 min-w-0">
                    {icon}
                    <div className="min-w-0">
                      {title}
                      {subtitle}
                    </div>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    {headerActions}
                    <button
                      onClick={onClose}
                      className="flex items-center justify-center w-8 h-8 rounded-full glass-card text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
                      aria-label={LAYOUT_COPY.modal.close}
                    >
                      <X size={18} />
                    </button>
                  </div>
                </div>
              )}

              <div className={cn("flex-1 overflow-y-auto overscroll-contain min-h-0 custom-scrollbar", bodyClassName)}>
                {children}
              </div>

              {footer && <div className="shrink-0">{footer}</div>}
            </motion.div>
          </div>
        )}
      </AnimatePresence>
    );

    if (position === "global" && typeof document !== "undefined") {
      return createPortal(modalNode, document.body);
    }

    return modalNode;
  }
);

Modal.displayName = "Modal";
