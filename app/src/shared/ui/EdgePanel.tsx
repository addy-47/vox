import React, { memo, useRef } from "react";
import { X } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { Tooltip } from "@/shared/ui/Tooltip";

interface EdgePanelProps {
  side: "left" | "right";
  open: boolean;
  onClose: () => void;
  title: string;
  headerActions?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}

const EdgePanelInner = memo(
  ({ side, open, onClose, title, headerActions, className, children }: EdgePanelProps) => {
    const panelRef = useRef<HTMLElement>(null);

    // Escape key or clicking outside dismisses the panel
    useOverlay({
      onClose,
      ref: panelRef,
      dismissOnOutside: true,
      active: open,
    });

    const isLeft = side === "left";

    return (
      <AnimatePresence>
        {open && (
          <motion.aside
            ref={panelRef}
            role="dialog"
            aria-label={title}
            initial={{ x: isLeft ? "-100%" : "100%" }}
            animate={{ x: 0 }}
            exit={{ x: isLeft ? "-100%" : "100%" }}
            transition={{ duration: 0.24, ease: [0.16, 1, 0.3, 1] }}
            className={cn(
              "absolute top-0 bottom-0 z-[45] flex flex-col bg-[rgb(var(--card))]/95 backdrop-blur-2xl overflow-hidden pointer-events-auto select-auto",
              isLeft
                ? "left-0 border-r border-[rgba(var(--border),0.18)] shadow-[8px_0_36px_rgba(0,0,0,0.45)]"
                : "right-0 border-l border-[rgba(var(--border),0.18)] shadow-[-8px_0_36px_rgba(0,0,0,0.45)]",
              "w-[360px] max-w-[90vw]",
              className
            )}
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 h-12 shrink-0 border-b border-[rgba(var(--border),0.12)] bg-[rgba(var(--background),0.35)]">
              <h2 className="font-display text-[14.5px] font-bold tracking-wide text-[rgb(var(--foreground))] truncate">
                {title}
              </h2>
              <div className="flex items-center gap-1 shrink-0 ml-2">
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

            {/* Content Body — stops short of the bottom edge with consistent spacing */}
            <div className={cn("flex-1 min-h-0 flex flex-col overflow-hidden", isLeft ? "pb-6" : "pb-8 sm:pb-10")}>
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
