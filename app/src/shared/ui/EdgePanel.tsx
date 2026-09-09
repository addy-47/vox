import React, { memo, useRef } from "react";
import { X } from "lucide-react";
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
    const panelRef = useRef<HTMLDivElement>(null);

    useOverlay({
      onClose,
      ref: panelRef,
      dismissOnOutside: true,
      active: open,
    });

    if (!open) return null;

    const isLeft = side === "left";

    return (
      <aside
        ref={panelRef}
        role="dialog"
        aria-label={title}
        className={cn(
          "absolute z-40 flex flex-col glass-card border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.6)] backdrop-blur-xl shadow-2xl overflow-hidden pointer-events-auto",
          isLeft ? "left-4 top-[64px] bottom-[calc(72px+clamp(12px,2.5vh,28px))]" : "right-4 top-[64px] bottom-[calc(72px+clamp(12px,2.5vh,28px))]",
          "w-[min(22vw,320px)] min-w-[240px] max-w-[280px]",
          className
        )}
      >
        <div className="flex items-center justify-between px-4 pt-3 pb-2 shrink-0">
          <h2 className="font-display text-[15px] font-bold tracking-wide text-[rgb(var(--foreground))]">
            {title}
          </h2>
          <div className="flex items-center gap-1">
            {headerActions}
            <Tooltip label="Close" side="bottom">
              <button
                onClick={onClose}
                className="flex items-center justify-center w-8 h-8 rounded-full text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.06)] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                aria-label="Close panel"
              >
                <X size={16} />
              </button>
            </Tooltip>
          </div>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar">
          {children}
        </div>
      </aside>
    );
  }
);
EdgePanelInner.displayName = "EdgePanel";

export const EdgePanel = EdgePanelInner;
