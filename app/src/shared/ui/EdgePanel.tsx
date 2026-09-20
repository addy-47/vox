import React, { memo, useRef, useCallback, useEffect } from "react";
import { X } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { Tooltip } from "@/shared/ui/Tooltip";

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
  }: EdgePanelProps) => {
    const panelRef = useRef<HTMLElement>(null);

    // Escape key or clicking outside dismisses the panel, but preserves opposing edge panels
    const shouldDismissOnPointerDown = useCallback((target: Node) => {
      const el = panelRef.current;
      if (!el || !(target instanceof Element)) return false;
      if (el.contains(target)) return false;

      // Clicking inside any edge panel (e.g. opposing side) must NOT dismiss this panel
      if (target.closest("[data-edge-panel]")) {
        return false;
      }
      // Clicking on an edge trigger button must NOT dismiss this panel
      if (target.closest("[data-edge-trigger]")) {
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
        focusTimerRef.current = setTimeout(() => {
          const el = panelRef.current;
          if (el) {
            const focusable = el.querySelector<HTMLElement>(
              'button:not([disabled]),[tabIndex="0"],input,textarea,select,[contenteditable]'
            );
            (focusable || el).focus();
          }
        }, 60);
        cleanup = () => { if (focusTimerRef.current) clearTimeout(focusTimerRef.current); };
      } else if (focusedBeforeRef.current) {
        try { focusedBeforeRef.current.focus(); } catch {}
        focusedBeforeRef.current = null;
      }
      return cleanup;
    }, [open]);

    const isLeft = side === "left";

    // Vertical mask (top & bottom soft fade) — single-pass hardware-accelerated without composite intersection
    const maskStyles: React.CSSProperties = {
      WebkitMaskImage:
        "linear-gradient(to bottom, transparent 0px, black 16px, black calc(100% - 32px), transparent 100%)",
      maskImage:
        "linear-gradient(to bottom, transparent 0px, black 16px, black calc(100% - 32px), transparent 100%)",
    };

    return (
      <AnimatePresence>
        {open && (
          <motion.aside
            ref={panelRef}
            role="dialog"
            aria-label={title || "Panel"}
            data-edge-panel={side}
            initial={{ opacity: 0, x: isLeft ? "-100%" : "100%" }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: isLeft ? "-100%" : "100%" }}
            transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
            className={cn(
              "absolute top-0 bottom-0 z-[35] flex flex-col bg-[rgb(var(--card))]/90 backdrop-blur-md overflow-hidden pointer-events-auto select-auto border-[rgba(var(--border),0.06)]",
              isLeft ? "left-0 border-r" : "right-0 border-l",
              "w-[330px] max-w-[92vw]",
              className
            )}
          >
            {/* Header: Minimal anchor row or classic titled bar */}
            {minimalHeader ? (
              /* When panel is LEFT: close button on the RIGHT so it doesn't collide with the trigger.
                 When panel is RIGHT: close button on the LEFT (standard convention). */
              <div className={`flex items-center px-5 pt-4 pb-2 h-14 shrink-0 ${isLeft ? "flex-row-reverse" : ""}`}>
                <div className="flex items-center gap-1.5 shrink-0">
                  <Tooltip label="Close" side={isLeft ? "left" : "right"}>
                    <button
                      onClick={onClose}
                      className="flex items-center justify-center w-8 h-8 rounded-lg text-[rgb(var(--foreground-muted))]/70 hover:bg-[rgba(var(--foreground),0.06)] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                      aria-label="Close panel"
                    >
                      <X size={16} />
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
                  <Tooltip label="Close" side="bottom">
                    <button
                      onClick={onClose}
                      className="flex items-center justify-center w-7 h-7 rounded-lg text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.06)] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                      aria-label="Close panel"
                    >
                      <X size={15} />
                    </button>
                  </Tooltip>
                </div>
              </div>
            )}

            {/* Content Body — extends down smoothly with vertical fade mask for scrolling contents */}
            <div className="flex-1 min-h-0 flex flex-col overflow-hidden" style={maskStyles}>
              {children}
            </div>
          </motion.aside>
        )}
      </AnimatePresence>
    );
  }
);
EdgePanelInner.displayName = "EdgePanel";

export const EdgePanel = EdgePanelInner;
