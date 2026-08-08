import React from "react";
import { cn } from "@/shared/lib/utils";

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  children: React.ReactNode;
  variant?: "default" | "accent" | "mono" | "success" | "warning" | "danger" | "purple";
  size?: "xs" | "sm" | "md";
  icon?: React.ElementType;
}

export const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "default",
  size = "sm",
  icon: Icon,
  className,
  ...props
}) => {
  const getVariantStyles = () => {
    switch (variant) {
      case "accent":
        return "bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/20";
      case "mono":
        return "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground))]/70 border-[rgba(var(--foreground),0.04)] font-mono";
      case "success":
        return "bg-emerald-500/15 text-emerald-400 border-emerald-500/30 font-mono";
      case "warning":
        return "bg-amber-500/15 text-amber-300 border-amber-500/30 font-mono";
      case "danger":
        return "bg-rose-500/20 text-rose-200 border-rose-500/30";
      case "purple":
        return "bg-purple-500/15 text-purple-300 border-purple-500/30 font-mono";
      default:
        return "bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 border-[rgba(var(--border),0.08)]";
    }
  };

  const getSizeStyles = () => {
    switch (size) {
      case "xs":
        return "text-[9px] px-1.5 py-0.5 rounded";
      case "md":
        return "text-[11px] px-2.5 py-1 rounded-lg";
      case "sm":
      default:
        return "text-[10px] px-2 py-0.5 rounded-md";
    }
  };

  return (
    <span
      className={cn(
        "font-bold uppercase tracking-wider inline-flex items-center gap-1 border leading-none shrink-0 transition-colors",
        getVariantStyles(),
        getSizeStyles(),
        className
      )}
      {...props}
    >
      {Icon && <Icon size={size === "xs" ? 10 : size === "md" ? 14 : 12} />}
      {children}
    </span>
  );
};
