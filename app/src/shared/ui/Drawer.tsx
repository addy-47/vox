import { memo, useState, useEffect, useRef, useCallback } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { Tooltip } from "./Tooltip";


export interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  subtitle?: React.ReactNode;
  icon?: React.ReactNode;
  /** Actions rendered on the right side of the header, before the close button. */
  headerActions?: React.ReactNode;
  /**
   * "page" — the drawer layers under app chrome (default z-30), used for
   * in-page detail surfaces. "global" — layers above app chrome (default z-50),
   * used for app-level drawers.
   */
  position?: "page" | "global";
  /** Initial height as a percentage of the viewport. */
  height?: number;
  minHeight?: number;
  maxHeight?: number;
  resizable?: boolean;
  withHandle?: boolean;
  /** Renders a dark blurred backdrop; clicking it closes the drawer. */
  backdrop?: boolean;
  zIndex?: number;
  /** Tooltip label on the resize handle (e.g. "Drag to resize · double-click to expand"). */
  resizeHint?: string;
  /** Fixed footer rendered below the scrollable body (shrink-0). */
  footer?: React.ReactNode;
  ariaLabel?: string;
  className?: string;
  bodyClassName?: string;
  children: React.ReactNode;
}

const DEFAULT_HEIGHT = 62;
const MIN_HEIGHT = 35;
const MAX_HEIGHT = 85;

/**
 * The single bottom-sheet drawer used across Vox for secondary work surfaces
 * (History transcript, Memory pipeline, Profiler, future Help). Provides the
 * unified overlay contract: backdrop dismiss, Escape (via the global overlay
 * stack), drag-handle resize, double-click expand, and focus management.
 */
export const Drawer = memo(
  ({
    open,
    onClose,
    title,
    subtitle,
    icon,
    headerActions,
    position = "page",
    height = DEFAULT_HEIGHT,
    minHeight = MIN_HEIGHT,
    maxHeight = MAX_HEIGHT,
    resizable = true,
    withHandle = true,
    backdrop = true,
    zIndex,
    resizeHint,
    footer,
    ariaLabel = "Drawer",
    className,
    bodyClassName,
    children,
  }: DrawerProps) => {
    const [heightPercent, setHeightPercent] = useState(height);
    const [isDragging, setIsDragging] = useState(false);
    const sheetRef = useRef<HTMLDivElement>(null);
    const currentHeightRef = useRef(heightPercent);
    currentHeightRef.current = heightPercent;

    useEffect(() => {
      if (open) setHeightPercent(height);
    }, [open, height]);

    // Register with the global overlay stack so Escape closes this drawer.
    useOverlay({
      onClose,
      active: open,
      ref: sheetRef,
      dismissOnOutside: false, // backdrop handles outside clicks
    });

    // Focus the sheet on open; restore focus to the trigger on close.
    const previouslyFocusedRef = useRef<HTMLElement | null>(null);
    useEffect(() => {
      if (!open) return;
      previouslyFocusedRef.current = document.activeElement as HTMLElement | null;
      sheetRef.current?.focus();
      return () => {
        previouslyFocusedRef.current?.focus?.();
      };
    }, [open]);

    const handleDragStart = useCallback(
      (e: React.PointerEvent<HTMLDivElement>) => {
        if (!resizable) return;
        e.preventDefault();
        e.stopPropagation();
        const target = e.currentTarget;
        target.setPointerCapture(e.pointerId);

        setIsDragging(true);
        const startY = e.clientY;
        const startHeight = currentHeightRef.current;
        let lastHeight = startHeight;

        const onPointerMove = (moveEvent: PointerEvent) => {
          const deltaPx = startY - moveEvent.clientY;
          const deltaPercent = (deltaPx / window.innerHeight) * 100;
          lastHeight = Math.min(maxHeight, Math.max(minHeight, startHeight + deltaPercent));
          if (sheetRef.current) {
            sheetRef.current.style.height = `${lastHeight}%`;
          }
        };

        const onPointerUp = (upEvent: PointerEvent) => {
          upEvent.preventDefault();
          upEvent.stopPropagation();
          setIsDragging(false);
          setHeightPercent(lastHeight);
          try {
            target.releasePointerCapture(upEvent.pointerId);
          } catch {}
          target.removeEventListener("pointermove", onPointerMove);
          target.removeEventListener("pointerup", onPointerUp);
          target.removeEventListener("pointercancel", onPointerUp);
        };

        target.addEventListener("pointermove", onPointerMove);
        target.addEventListener("pointerup", onPointerUp);
        target.addEventListener("pointercancel", onPointerUp);
      },
      [resizable, minHeight, maxHeight]
    );

    const handleToggleExpand = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        setHeightPercent((prev) => (prev > 70 ? height : maxHeight));
      },
      [height, maxHeight]
    );

    const effectiveZ = zIndex ?? (position === "global" ? 60 : 30);

    const drawerNode = (
      <AnimatePresence>
        {open && (
          <div
            className={cn("fixed inset-0 z-[var(--drawer-z)] pointer-events-none", className)}
            style={{ ["--drawer-z" as string]: effectiveZ }}
          >
            {backdrop && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.2 }}
                onClick={onClose}
                className="absolute inset-0 bg-[rgb(var(--background))]/60 backdrop-blur-sm pointer-events-auto cursor-default"
              />
            )}

            <motion.div
              ref={sheetRef}
              role="dialog"
              aria-modal="true"
              aria-label={ariaLabel}
              tabIndex={-1}
              initial={{ y: "100%" }}
              animate={{ y: 0 }}
              exit={{ y: "100%" }}
              transition={{ duration: 0.38, ease: [0.16, 1, 0.3, 1] }}
              style={{ height: `${heightPercent}%` }}
              className={cn(
                "absolute bottom-0 left-0 right-0 flex flex-col rounded-t-3xl overflow-hidden border-0 shadow-2xl outline-none pointer-events-auto",
                isDragging ? "select-none transition-none" : "transition-[height] duration-150 ease-out"
              )}
              onClick={(e) => e.stopPropagation()}
            >
              {withHandle && (
                <Tooltip label={resizeHint} wrapperClassName="w-full shrink-0">
                  <div
                    onPointerDown={handleDragStart}
                    onDoubleClick={handleToggleExpand}
                    role="separator"
                    aria-orientation="horizontal"
                    aria-label={resizeHint ?? "Resize drawer"}
                    className="w-full h-5 flex items-center justify-center cursor-row-resize group hover:bg-[rgb(var(--accent))]/5 transition-colors touch-none shrink-0"
                  >
                    <div className="w-12 h-1 rounded-full bg-[rgba(var(--accent),0.3)] group-hover:bg-[rgb(var(--accent))] transition-colors shadow-sm" />
                  </div>
                </Tooltip>
              )}

              {(title || icon || headerActions) && (
                <div className="flex items-center justify-between px-6 pt-1 pb-3 border-b border-[rgba(var(--accent),0.08)] shrink-0">
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
                      aria-label="Close drawer"
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
      return createPortal(drawerNode, document.body);
    }

    return drawerNode;
  }
);


Drawer.displayName = "Drawer";