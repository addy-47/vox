/**
 * INVARIANT: Tooltip Mobile Touch Behavior
 * On touch / mobile devices, tooltips MUST NOT appear on tap for action buttons,
 * menu items, or interactive controls (`mobileTapEnabled: false` by default).
 * Showing tooltips on mobile taps intercepts clicks, causing UI glitches.
 * Only explicit informational icons (e.g. HelpCircle, info pills) may set
 * `mobileTapEnabled: true`.
 */

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
  useClick,
  useFocus,
  useDismiss,
  useRole,
  useInteractions,
  type Placement,
} from "@floating-ui/react";
import { cn } from "@/shared/lib/utils";
import { getShortcutById } from "@/data/shortcuts";
import { isSpatialNavigating, subscribeSpatialNavigating } from "@/shared/lib/spatialNavigation";
import { isRealMobileDevice } from "@/layout/breakpoints";

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
  asChild?: boolean;
  /** Whether tapping the target on mobile/touch should toggle the tooltip. Default: false. Only enable for dedicated info triggers (e.g. LLM capabilities, badges). */
  mobileTapEnabled?: boolean;
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
  asChild = false,
  mobileTapEnabled = false,
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

  const isTouchOrMobile = useMemo(() => {
    if (typeof window === "undefined") return false;
    return isRealMobileDevice() || (typeof window.matchMedia === "function" && window.matchMedia("(pointer: coarse)").matches);
  }, []);

  const hover = useHover(context, {
    move: false,
    delay: { open: 200, close: 100 },
    enabled: !isTouchOrMobile,
  });
  const click = useClick(context, {
    enabled: isTouchOrMobile && mobileTapEnabled,
  });
  const focus = useFocus(context, {
    enabled: !isTouchOrMobile,
  });
  const dismiss = useDismiss(context);
  const role = useRole(context, { role: "tooltip" });

  const { getReferenceProps, getFloatingProps } = useInteractions([
    hover,
    click,
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

  const renderReference = () => {
    if (asChild && React.isValidElement(children)) {
      const child = children as React.ReactElement<Record<string, unknown>>;
      const childWithRef = child as unknown as { ref?: React.Ref<HTMLElement | SVGSVGElement> };
      const existingRef = childWithRef.ref;
      const mergedRef = (node: HTMLElement | SVGSVGElement | null) => {
        refs.setReference(node);
        if (typeof existingRef === "function") {
          existingRef(node);
        } else if (existingRef && typeof existingRef === "object" && "current" in existingRef) {
          (existingRef as React.MutableRefObject<HTMLElement | SVGSVGElement | null>).current = node;
        }
      };

      const childClassName = typeof child.props.className === "string" ? child.props.className : undefined;
      const childStyle = typeof child.props.style === "object" && child.props.style !== null ? (child.props.style as React.CSSProperties) : undefined;

      const referenceProps = getReferenceProps({
        ...child.props,
        ref: mergedRef,
        className: wrapperClassName
          ? cn(childClassName, wrapperClassName)
          : childClassName,
        style: wrapperStyle
          ? { ...childStyle, ...wrapperStyle }
          : childStyle,
      });

      return React.cloneElement(child, referenceProps);
    }

    return (
      <span
        ref={refs.setReference}
        {...getReferenceProps()}
        className={cn("relative inline-flex items-center", wrapperClassName)}
        style={wrapperStyle}
      >
        {children}
      </span>
    );
  };

  return (
    <>
      {renderReference()}
      {isOpen && !spatialNavigating && (
        <FloatingPortal>
          <div
            ref={refs.setFloating}
            style={floatingStyles}
            {...getFloatingProps()}
            className={cn(
              "z-[9999] pointer-events-none flex items-center gap-1.5 max-w-[320px] w-max select-none rounded-md px-2 py-1 text-[12px] font-medium leading-snug shadow-lg transition-opacity duration-150 animate-fade-in",
              "bg-[rgb(var(--card))]/95 text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.2)]",
              // E3: compact treatment on touch/coarse-pointer devices — smaller
              // footprint so a tap-triggered tooltip never covers the screen.
              isTouchOrMobile && "max-w-[240px] px-1.5 text-[11px]",
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
            {typeof label === "string" ? (
              <span className="flex-1 min-w-0 break-words">{label}</span>
            ) : (
              <div className="flex-1 min-w-0 w-full">{label}</div>
            )}
            {resolvedShortcut && (
              <kbd className="font-mono text-[11px] tracking-tight text-[rgb(var(--foreground-muted))]/70 shrink-0">
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