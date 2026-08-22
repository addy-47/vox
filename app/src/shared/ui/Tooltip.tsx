import React, { useState, useRef, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/shared/lib/utils";

interface TooltipProps {
  label: React.ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  align?: "start" | "center" | "end";
  className?: string;
  wrapperClassName?: string;
  wrapperStyle?: React.CSSProperties;
  children: React.ReactNode;
}

export const Tooltip: React.FC<TooltipProps> = ({
  label,
  side = "top",
  align = "center",
  className,
  wrapperClassName,
  wrapperStyle,
  children,
}) => {
  const [isVisible, setIsVisible] = useState(false);
  const triggerRef = useRef<HTMLSpanElement>(null);
  const tooltipRef = useRef<HTMLSpanElement>(null);

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const tooltipEl = tooltipRef.current;
    const tooltipWidth = tooltipEl ? tooltipEl.offsetWidth : 160;
    const tooltipHeight = tooltipEl ? tooltipEl.offsetHeight : 36;

    let top = 0;
    let left = 0;

    let effectiveSide = side;
    // Auto flip to bottom if top would be clipped
    if (side === "top" && rect.top - tooltipHeight - 8 < 10) {
      effectiveSide = "bottom";
    }

    if (effectiveSide === "top") {
      top = rect.top - tooltipHeight - 8;
      if (align === "start") left = rect.left;
      else if (align === "end") left = rect.right - tooltipWidth;
      else left = rect.left + rect.width / 2 - tooltipWidth / 2;
    } else if (effectiveSide === "bottom") {
      top = rect.bottom + 8;
      if (align === "start") left = rect.left;
      else if (align === "end") left = rect.right - tooltipWidth;
      else left = rect.left + rect.width / 2 - tooltipWidth / 2;
    } else if (effectiveSide === "left") {
      top = rect.top + rect.height / 2 - tooltipHeight / 2;
      left = rect.left - tooltipWidth - 8;
    } else if (effectiveSide === "right") {
      top = rect.top + rect.height / 2 - tooltipHeight / 2;
      left = rect.right + 8;
    }

    // Keep within screen horizontally
    const minLeft = 10;
    const maxLeft = typeof window !== "undefined" ? window.innerWidth - tooltipWidth - 10 : 800;
    left = Math.max(minLeft, Math.min(maxLeft, left));

    if (tooltipRef.current) {
      tooltipRef.current.style.top = `${Math.round(top)}px`;
      tooltipRef.current.style.left = `${Math.round(left)}px`;
    }
  }, [side, align]);

  const handleMouseEnter = () => {
    setIsVisible(true);
  };

  const handleMouseLeave = () => {
    setIsVisible(false);
  };

  useEffect(() => {
    if (!isVisible) return undefined;
    updatePosition();
    const handleScroll = () => updatePosition();
    window.addEventListener("scroll", handleScroll, true);
    window.addEventListener("resize", handleScroll);
    return () => {
      window.removeEventListener("scroll", handleScroll, true);
      window.removeEventListener("resize", handleScroll);
    };
  }, [isVisible, updatePosition]);

  return (
    <span
      ref={triggerRef}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onFocus={handleMouseEnter}
      onBlur={handleMouseLeave}
      className={cn("relative inline-flex", wrapperClassName)}
      style={wrapperStyle}
    >
      {children}
      {isVisible &&
        typeof document !== "undefined" &&
        createPortal(
          <span
            ref={tooltipRef}
            role="tooltip"
            style={{
              position: "fixed",
            }}
            className={cn(
              "pointer-events-none z-[9999] max-w-[220px] w-max whitespace-normal break-words rounded-xl border border-[rgba(var(--foreground),0.14)] bg-[rgb(var(--card))]/98 text-[rgb(var(--foreground))] shadow-2xl backdrop-blur-2xl px-2.5 py-1 text-[11px] font-medium leading-snug animate-fade-in text-center",
              className
            )}
          >
            {label}
          </span>,
          document.body
        )}
    </span>
  );
};