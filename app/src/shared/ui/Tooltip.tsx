import React, { useState, useRef, useEffect, useMemo } from "react";
import {
  useFloating,
  autoUpdate,
  offset,
  flip,
  shift,
  arrow,
  FloatingArrow,
  FloatingPortal,
  useHover,
  useFocus,
  useDismiss,
  useRole,
  useInteractions,
  type Placement,
} from "@floating-ui/react";
import { cn } from "@/shared/lib/utils";
import { getShortcutById } from "@/data/shortcuts";
import { isSpatialNavigating, subscribeSpatialNavigating } from "@/shared/lib/spatialNavigation";

export interface TooltipProps {
  label: React.ReactNode;
  shortcut?: string;
  shortcutId?: string;
  side?: "top" | "bottom" | "left" | "right";
  align?: "start" | "center" | "end";
  className?: string;
  wrapperClassName?: string;
  wrapperStyle?: React.CSSProperties;
  children: React.ReactNode;
  disabled?: boolean;
}

export const Tooltip: React.FC<TooltipProps> = React.memo(({
  label,
  shortcut,
  shortcutId,
  side = "top",
  align = "center",
  className,
  wrapperClassName,
  wrapperStyle,
  children,
  disabled = false,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const arrowRef = useRef<SVGSVGElement>(null);
  const [spatialNavigating, setSpatialNavigating] = useState(isSpatialNavigating());

  useEffect(() => {
    return subscribeSpatialNavigating((navigating) => {
      setSpatialNavigating(navigating);
      if (navigating) setIsOpen(false);
    });
  }, []);

  const placement = useMemo<Placement>(() => {
    if (align === "start") return `${side}-start` as Placement;
    if (align === "end") return `${side}-end` as Placement;
    return side as Placement;
  }, [side, align]);

  const { refs, floatingStyles, context } = useFloating({
    open: isOpen && !spatialNavigating && !disabled,
    onOpenChange: (open) => {
      if (spatialNavigating && open) return;
      setIsOpen(open);
    },
    placement,
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(8),
      flip({ fallbackAxisSideDirection: "start" }),
      shift({ padding: 8 }),
      arrow({ element: arrowRef }),
    ],
  });

  const hover = useHover(context, {
    move: false,
    delay: { open: 200, close: 100 },
  });
  const focus = useFocus(context);
  const dismiss = useDismiss(context);
  const role = useRole(context, { role: "tooltip" });

  const { getReferenceProps, getFloatingProps } = useInteractions([
    hover,
    focus,
    dismiss,
    role,
  ]);

  const resolvedShortcut = useMemo(() => {
    if (shortcut) return shortcut;
    if (shortcutId) {
      const def = getShortcutById(shortcutId);
      return def?.keys;
    }
    return undefined;
  }, [shortcut, shortcutId]);

  if (disabled || !label) {
    return <>{children}</>;
  }

  return (
    <>
      <span
        ref={refs.setReference}
        {...getReferenceProps()}
        className={cn("relative inline-flex items-center", wrapperClassName)}
        style={wrapperStyle}
      >
        {children}
      </span>
      {isOpen && !spatialNavigating && (
        <FloatingPortal>
          <div
            ref={refs.setFloating}
            style={floatingStyles}
            {...getFloatingProps()}
            className={cn(
              "z-[9999] pointer-events-none flex items-center gap-2 max-w-[280px] w-max select-none rounded-lg px-2.5 py-1.5 text-[11px] font-medium leading-tight shadow-xl backdrop-blur-md transition-opacity duration-150 animate-fade-in",
              "bg-[rgb(var(--card))]/95 text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.2)]",
              className
            )}
          >
            <FloatingArrow
              ref={arrowRef}
              context={context}
              className="fill-[rgb(var(--card))] stroke-[rgba(var(--accent),0.2)] stroke-1"
              width={10}
              height={5}
            />
            <span className="truncate">{label}</span>
            {resolvedShortcut && (
              <kbd className="px-1.5 py-0.5 rounded text-[10px] font-mono font-semibold tracking-tight bg-white/10 text-[rgb(var(--foreground-muted))] border border-white/15 shadow-xs shrink-0">
                {resolvedShortcut}
              </kbd>
            )}
          </div>
        </FloatingPortal>
      )}
    </>
  );
});

Tooltip.displayName = "Tooltip";