import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface ProgressBarProps {
  label: string;
  textRef?: React.RefObject<HTMLSpanElement | null>;
  barRef?: React.RefObject<HTMLDivElement | null>;
  value?: number;
  initialText?: string;
  className?: string;
  size?: "sm" | "md";
}

export const ProgressBar = memo<ProgressBarProps>(
  ({
    label,
    textRef,
    barRef,
    value = 0,
    initialText = "0.0%",
    className,
    size = "sm",
  }) => {
    const isSm = size === "sm";

    return (
      <div className={cn("space-y-1.5 w-full", className)}>
        <div className="flex justify-between items-baseline">
          <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            {label}
          </span>
          <span
            ref={textRef}
            className="text-[13px] font-mono font-bold text-[rgb(var(--foreground))]"
          >
            {initialText}
          </span>
        </div>
        <div
          className={cn(
            "w-full rounded-full bg-[rgba(var(--foreground),0.06)] overflow-hidden",
            isSm ? "h-[3px]" : "h-[4px]"
          )}
        >
          <div
            ref={barRef}
            className="h-full rounded-full bg-[rgb(var(--accent))] transition-all duration-150"
            style={{ width: `${Math.min(100, Math.max(0, value))}%` }}
          />
        </div>
      </div>
    );
  }
);

ProgressBar.displayName = "ProgressBar";
