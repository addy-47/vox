import React, { memo, useRef, useCallback, useEffect } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { Tooltip } from "@/shared/ui/Tooltip";
import { OverlayBackdrop } from "@/shared/ui/OverlayBackdrop";
import { BottomDockFeather } from "@/shared/ui/BottomDockFeather";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { useViewportResize } from "@/shared/hooks/useViewportResize";

export interface EdgePanelProps {
  side: "left" | "right";
  open: boolean;
  onClose: () => void;
  title?: string;
  headerActions?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
  /**
   * When true (default for redesigned session rail), renders a minimal header
   * without prominent banner borders, designed to open seamlessly from behind
   * the docked corner trigger icon.
   */
  minimalHeader?: boolean;
  /**
   * "page" — the panel layers under app chrome (default z-35).
   * "global" — rendered in document.body via createPortal (default z-70),
   * layering above all chrome and clusters.
   */
  position?: "page" | "global";
  zIndex?: number;
}

const EdgePanelInner = memo(
  ({
    side,
    open,
    onClose,
    title,
    headerActions,
    className,
    children,
    minimalHeader = false,
    position = "page",
    zIndex,
  }: EdgePanelProps) => {
    const panelRef = useRef<HTMLElement>(null);
    const { isMobile } = useViewportResize();

    // Escape key or clicking outside dismisses the panel, but preserves opposing edge panels
    const shouldDismissOnPointerDown = useCallback((target: Node) => {
      const el = panelRef.current;
      if (!el || !(target instanceof Element)) return false;
      if (el.contains(target)) return false;

      // Clicking inside any edge panel (e.g. opposing side) must NOT dismiss this panel
      if (target.closest("[data-edge-panel]")) {
        return false;
      }
      // Clicking on an edge trigger button or top-right cluster must NOT dismiss this panel
      if (target.closest("[data-edge-trigger]") || target.closest("[data-spatial-zone='cluster']")) {
        return false;
      }
      // Clicking inside a context menu portal (rendered in body) must NOT dismiss this panel
      if (target.closest("[data-context-menu]")) {
        return false;
      }

      return true;
    }, []);

    useOverlay({
      onClose,
      ref: panelRef,
      dismissOnOutside: true,
      shouldDismissOnPointerDown,
      active: open,
    });

    // Focus-in on open, restore focus on close
    const focusedBeforeRef = useRef<HTMLElement | null>(null);
    const focusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    useEffect(() => {
      let cleanup: (() => void) | undefined;
      if (open) {
        focusedBeforeRef.current = document.activeElement as HTMLElement | null;
        // Wait until after the 220ms slide-in animation settles to prevent layout jerk
        focusTimerRef.current = setTimeout(() => {
          const el = panelRef.current;
          if (el) {
            const focusable = el.querySelector<HTMLElement>(
              'button:not([disabled]),[tabIndex="0"],input,textarea,select,[contenteditable]'
            );
            try {
              if (focusable) {
                focusable.focus({ preventScroll: true });
              } else {
                el.focus({ preventScroll: true });
              }
            } catch {}
          }
        }, 240);
        cleanup = () => { if (focusTimerRef.current) clearTimeout(focusTimerRef.current); };
      } else if (focusedBeforeRef.current) {
        const prev = focusedBeforeRef.current;
        focusedBeforeRef.current = null;
        try {
          if (document.contains(prev)) prev.focus({ preventScroll: true });
        } catch {}
      }
      return cleanup;
    }, [open]);

    const isLeft = side === "left";
    const isGlobal = position === "global";
    const effectiveZ = zIndex ?? (isGlobal ? 70 : 35);
    const effectiveBackdropZ = effectiveZ - 1;

    const panelNode = (
      <AnimatePresence>
        {open && (
          <>
            {/* Backdrop dimming: shared OverlayBackdrop (single source of truth). */}
            <OverlayBackdrop
              onClose={onClose}
              zIndex={effectiveBackdropZ}
              className={cn(!isGlobal && "lg:hidden")}
            />
            <motion.aside
              ref={panelRef}
              role="dialog"
              aria-label={title || LAYOUT_COPY.panel.fallback}
              data-edge-panel={side}
              initial={{ x: isLeft ? "-100%" : "100%" }}
              animate={{ x: 0 }}
              exit={{ x: isLeft ? "-100%" : "100%" }}
              transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
              style={{ zIndex: effectiveZ }}
              className={cn(
                isGlobal ? "fixed top-0 bottom-0" : "absolute top-0 bottom-0",
                "flex flex-col bg-[rgb(var(--card))]/96 overflow-hidden pointer-events-auto select-auto border-[rgba(var(--border),0.08)] shadow-lg transform-gpu",
                isLeft ? "left-0 border-r" : "right-0 border-l",
                "w-[340px] max-w-[92vw]",
                className
              )}
            >
            {/* Header: Minimal anchor row or classic titled bar */}
            {minimalHeader ? (
              /* When panel is LEFT: close button on the RIGHT so it doesn't collide with the trigger.
                 When panel is RIGHT: close button on the LEFT (standard convention).
                 Padding and height match AppHeader exactly so the close button aligns with the header trigger buttons. */
              <div
                className={cn(
                  "flex items-center shrink-0",
                  isMobile
                    ? "pt-4 px-4 sm:px-8 pb-2.5"
                    : "h-14 px-5",
                  isLeft ? "flex-row-reverse" : ""
                )}
              >
                <div className="flex items-center gap-1.5 shrink-0">
                  <Tooltip label={LAYOUT_COPY.panel.close} side={isLeft ? "left" : "right"}>
                    <button
                      onClick={onClose}
                      className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] transition-colors cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
                      aria-label={LAYOUT_COPY.panel.close}
                    >
                      <X size={14} strokeWidth={1.75} />
                    </button>
                  </Tooltip>
                  {headerActions}
                </div>
                <div className="flex-1" aria-hidden="true" />
              </div>
            ) : (
              <div className="flex items-center justify-between px-4 py-3 h-12 shrink-0 border-b border-[rgba(var(--border),0.08)] bg-[rgba(var(--background),0.2)]">
                {title && (
                  <h2 className="font-display text-[14px] font-bold tracking-wide text-[rgb(var(--foreground))] truncate">
                    {title}
                  </h2>
                )}
                <div className="flex items-center gap-1 shrink-0 ml-auto">
                  {headerActions}
                  <Tooltip label={LAYOUT_COPY.panel.close} side="bottom">
                    <button
                      onClick={onClose}
                      className="flex items-center justify-center w-7 h-7 rounded-lg text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.06)] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                      aria-label={LAYOUT_COPY.panel.close}
                    >
                      <X size={15} />
                    </button>
                  </Tooltip>
                </div>
              </div>
            )}

            {/* Content Body — strictly self-contained with internal scroll dissolve */}
            <div className="flex-1 min-h-0 flex flex-col overflow-hidden relative">
              <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
                {children}
              </div>
              {/* Internal bottom dissolve strictly bounded to panel */}
              <BottomDockFeather className="absolute bottom-0 left-0 right-0 h-14 pointer-events-none z-10" />
            </div>
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  );

  if (isGlobal && typeof document !== "undefined") {
    return createPortal(panelNode, document.body);
  }
  if (typeof document !== "undefined") {
    const stageSlot = document.getElementById("stage-edge-panels");
    if (stageSlot) {
      return createPortal(panelNode, stageSlot);
    }
  }
  return panelNode;
  }
);
EdgePanelInner.displayName = "EdgePanel";

export const EdgePanel = EdgePanelInner;
