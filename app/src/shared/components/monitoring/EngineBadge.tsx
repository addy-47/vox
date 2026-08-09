import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { StatusDot } from "@/shared/ui";

interface EngineBadgeProps {
  label: string;
  active: boolean;
  icon: React.ReactNode;
}

export const EngineBadge = memo(({ label, active, icon }: EngineBadgeProps) => (
  <div
    className={cn(
      "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[11px] font-bold tracking-widest uppercase transition-all duration-500",
      active
        ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.1)]"
        : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.06)]"
    )}
  >
    <StatusDot status={active ? "active" : "offline"} size="sm" />
    <span className={cn("transition-transform duration-500", active && "scale-110")}>
      {icon}
    </span>
    {label}
  </div>
));
EngineBadge.displayName = "EngineBadge";
