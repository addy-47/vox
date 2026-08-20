import React from "react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "./Tooltip";

export interface SegmentedOption<T extends string = string> {
  id: T;
  label?: string;
  icon?: React.ElementType;
  title?: string;
}

export interface SegmentedControlProps<T extends string = string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  className?: string;
}

export function SegmentedControl<T extends string = string>({
  options,
  value,
  onChange,
  size = "sm",
  className,
}: SegmentedControlProps<T>) {
  return (
    <div
      className={cn(
        "flex bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.08)] p-0.5 rounded-xl gap-0.5 shrink-0 select-none",
        className
      )}
    >
      {options.map((opt) => {
        const isActive = value === opt.id;
        const Icon = opt.icon;

        const button = (
          <button
            type="button"
            onClick={() => onChange(opt.id)}
            aria-label={opt.title || opt.label || opt.id}
            className={cn(
              "transition-all duration-300 cursor-pointer border flex items-center justify-center font-bold",
              size === "sm" && "px-2.5 py-0.5 text-[11px] rounded-lg min-h-[26px]",
              size === "md" && "px-3 py-1 text-[12px] rounded-lg min-h-[30px]",
              isActive
                ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.1)] font-extrabold"
                : "bg-transparent border-transparent text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))]"
            )}
          >
            {Icon && <Icon size={size === "sm" ? 14 : 16} className={opt.label ? "mr-1" : ""} />}
            {opt.label && <span>{opt.label}</span>}
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
