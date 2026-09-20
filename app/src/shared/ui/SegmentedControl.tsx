import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "./Tooltip";

export interface SegmentedOption<T extends string = string> {
  id: T;
  label?: string;
  compactLabel?: string;
  icon?: React.ElementType;
  title?: string;
  disabled?: boolean;
}

export interface SegmentedControlProps<T extends string = string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  className?: string;
}

function SegmentedControlInner<T extends string = string>({
  options,
  value,
  onChange,
  size = "sm",
  className,
}: SegmentedControlProps<T>) {
  return (
    <div
      data-arrow-nav
      role="radiogroup"
      aria-label={className ?? "segmented control"}
      className={cn(
        "flex bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.08)] p-0.5 rounded-xl gap-0.5 shrink-0 select-none",
        className
      )}
      onKeyDown={(e) => {
        if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
        e.preventDefault();
        e.stopPropagation();
        const btns = Array.from(
          (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>(
            'button:not([disabled])'
          )
        );
        if (btns.length === 0) return;
        const currentIdx = btns.findIndex((b) => b === document.activeElement);
        let nextIdx: number;
        if (e.key === "ArrowRight") {
          nextIdx = currentIdx < btns.length - 1 ? currentIdx + 1 : 0;
        } else {
          nextIdx = currentIdx > 0 ? currentIdx - 1 : btns.length - 1;
        }
        const next = btns[nextIdx];
        next.focus();
        if (currentIdx !== nextIdx) {
          const optId = next.getAttribute("data-seg-id");
          if (optId !== null) onChange(optId as T);
        }
      }}
    >
      {options.map((opt) => {
        const isActive = value === opt.id;
        const Icon = opt.icon;
        const isDisabled = !!opt.disabled;

        const button = (
          <button
            type="button"
            data-seg-id={opt.id}
            disabled={isDisabled}
            onClick={() => !isDisabled && onChange(opt.id)}
            aria-selected={isActive}
            aria-disabled={isDisabled}
            aria-label={opt.title || opt.label || opt.id}
            className={cn(
              "transition-all duration-300 border flex items-center justify-center font-bold",
              size === "sm" && "px-2.5 py-0.5 text-[11px] rounded-lg min-h-[26px]",
              size === "md" && "px-3 py-1 text-[12px] rounded-lg min-h-[30px]",
              isDisabled
                ? "opacity-35 cursor-not-allowed bg-transparent border-transparent text-[rgb(var(--foreground-muted))]"
                : "cursor-pointer",
              !isDisabled && isActive
                ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.1)] font-extrabold"
                : !isDisabled
                ? "bg-transparent border-transparent text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))]"
                : ""
            )}
          >
            {Icon && <Icon size={size === "sm" ? 14 : 16} className={opt.label ? "mr-1 @[380px]:inline hidden" : ""} />}
            {opt.compactLabel && opt.label ? (
              <>
                <span className="inline @[380px]:hidden">{opt.compactLabel}</span>
                <span className="hidden @[380px]:inline">{opt.label}</span>
              </>
            ) : opt.label ? (
              <span>{opt.label}</span>
            ) : null}
          </button>
        );

        return (
          <React.Fragment key={opt.id}>
            {opt.title ? (
              <Tooltip label={opt.title}>{button}</Tooltip>
            ) : (
              button
            )}
          </React.Fragment>
        );
      })}
    </div>
  );
}

export const SegmentedControl = memo(SegmentedControlInner) as typeof SegmentedControlInner;
(SegmentedControl as React.FC).displayName = "SegmentedControl";
