import React from "react";
import { cn } from "@/shared/lib/utils";

interface TooltipProps {
  label: React.ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  className?: string;
  wrapperClassName?: string;
  wrapperStyle?: React.CSSProperties;
  children: React.ReactNode;
}

export const Tooltip: React.FC<TooltipProps> = ({
  label,
  side = "top",
  className,
  wrapperClassName,
  wrapperStyle,
  children,
}) => (
  <span
    className={cn("relative inline-flex group/tooltip", wrapperClassName)}
    style={wrapperStyle}
  >
    {children}
    <span
      role="tooltip"
      className={cn(
        "pointer-events-none absolute z-50 whitespace-nowrap rounded-lg border border-[rgba(var(--accent),0.15)] bg-[rgb(var(--background))]/95 backdrop-blur-xl px-2.5 py-1 text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] shadow-lg opacity-0 scale-95 transition-all duration-150 group-hover/tooltip:opacity-100 group-hover/tooltip:scale-100 group-focus-visible/tooltip:opacity-100 group-focus-visible/tooltip:scale-100",
        side === "top" && "bottom-full left-1/2 -translate-x-1/2 mb-2",
        side === "bottom" && "top-full left-1/2 -translate-x-1/2 mt-2",
        side === "left" && "right-full top-1/2 -translate-y-1/2 mr-2",
        side === "right" && "left-full top-1/2 -translate-y-1/2 ml-2",
        className
      )}
    >
      {label}
    </span>
  </span>
);