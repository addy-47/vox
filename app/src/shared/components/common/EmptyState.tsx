import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Card } from "../../ui/Card";

export interface EmptyStateProps {
  icon?: React.ElementType;
  title: React.ReactNode;
  description?: React.ReactNode;
  action?: React.ReactNode;
  variant?: "default" | "card" | "dashed";
  size?: "sm" | "md" | "lg";
  className?: string;
}

/**
 * Standard Vox Empty State Component.
 *
 * Provides a standardized visual presentation for zero-data states across all
 * cards, modals, lists, and pages without ad-hoc borders or forced overflow.
 *
 * - "default": Clean, borderless, frameless centering that integrates seamlessly
 *   into any parent container without double borders or unnecessary scrollbars.
 * - "card": Contained surface elevation with subtle card border.
 * - "dashed": Subtle dashed accent border for drop/upload zones or staging slots.
 */
export const EmptyState: React.FC<EmptyStateProps> = memo(({
  icon: Icon,
  title,
  description,
  action,
  variant = "default",
  size = "md",
  className,
}) => {
  const content = (
    <>
      {Icon && (
        <div
          className={cn(
            "rounded-2xl bg-[rgba(var(--accent),0.07)] border border-[rgba(var(--accent),0.15)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0",
            size === "sm" ? "w-9 h-9 mb-2" : size === "lg" ? "w-14 h-14 mb-4" : "w-11 h-11 mb-2.5"
          )}
        >
          <Icon size={size === "sm" ? 18 : size === "lg" ? 24 : 20} strokeWidth={1.8} />
        </div>
      )}
      <h4
        className={cn(
          "font-display font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/90 mb-1 leading-tight",
          size === "sm" ? "text-[12px]" : size === "lg" ? "text-[15px]" : "text-[13px]"
        )}
      >
        {title}
      </h4>
      {description && (
        <p
          className={cn(
            "text-[rgb(var(--foreground-muted))]/70 leading-relaxed max-w-xs",
            size === "sm" ? "text-[11px]" : size === "lg" ? "text-[13px]" : "text-[11.5px]"
          )}
        >
          {description}
        </p>
      )}
      {action && <div className="mt-2.5">{action}</div>}
    </>
  );

  if (variant === "card") {
    return (
      <Card
        elevation="surface"
        className={cn(
          "flex flex-col items-center justify-center text-center w-full select-none",
          size === "sm" ? "p-4" : size === "lg" ? "p-8 min-h-[220px]" : "p-6 min-h-[160px]",
          className
        )}
      >
        {content}
      </Card>
    );
  }

  if (variant === "dashed") {
    return (
      <div
        className={cn(
          "flex flex-col items-center justify-center text-center w-full rounded-2xl border border-dashed border-[rgba(var(--accent),0.18)] bg-[rgba(var(--card),0.2)] select-none",
          size === "sm" ? "p-3" : size === "lg" ? "p-8" : "p-5",
          className
        )}
      >
        {content}
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center text-center w-full h-full select-none",
        size === "sm" ? "p-2" : size === "lg" ? "p-6" : "p-4",
        className
      )}
    >
      {content}
    </div>
  );
});

EmptyState.displayName = "EmptyState";
