import React from "react";
import { cn } from "@/shared/lib/utils";

export type StatusType =
  | "active"
  | "idle"
  | "thinking"
  | "speaking"
  | "warning"
  | "error"
  | "offline";

export interface StatusDotProps {
  status: StatusType;
  label?: string;
  size?: "sm" | "md" | "lg";
  pulse?: boolean;
  className?: string;
}

export const StatusDot: React.FC<StatusDotProps> = ({
  status,
  label,
  size = "md",
  pulse = true,
  className,
}) => {
  const getStatusColor = () => {
    switch (status) {
      case "active":
      case "speaking":
        return "bg-emerald-400 border-emerald-500/40 text-emerald-400";
      case "thinking":
        return "bg-purple-400 border-purple-500/40 text-purple-400";
      case "warning":
        return "bg-amber-400 border-amber-500/40 text-amber-400";
      case "error":
        return "bg-rose-400 border-rose-500/40 text-rose-400";
      case "idle":
      case "offline":
      default:
        return "bg-[rgb(var(--foreground-muted))]/40 border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]/60";
    }
  };

  const getSizeClass = () => {
    switch (size) {
      case "sm":
        return "w-1.5 h-1.5";
      case "lg":
        return "w-2.5 h-2.5";
      case "md":
      default:
        return "w-2 h-2";
    }
  };

  return (
    <div className={cn("inline-flex items-center gap-1.5 select-none", className)}>
      <span className="relative flex items-center justify-center">
        {pulse && (status === "active" || status === "speaking" || status === "thinking") && (
          <span
            className={cn(
              "absolute inset-0 rounded-full animate-ping opacity-65",
              getStatusColor().split(" ")[0]
            )}
          />
        )}
        <span className={cn("rounded-full transition-colors", getStatusColor().split(" ")[0], getSizeClass())} />
      </span>
      {label && (
        <span className="text-[10px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80 leading-none">
          {label}
        </span>
      )}
    </div>
  );
};
