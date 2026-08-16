import React from "react";
import { cn } from "@/shared/lib/utils";
import { Card } from "../../ui/Card";

export interface EmptyStateProps {
  icon?: React.ElementType;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  icon: Icon,
  title,
  description,
  action,
  className,
}) => {
  return (
    <Card
      elevation="surface"
      className={cn(
        "p-8 flex flex-col items-center justify-center text-center w-full min-h-[200px] border-dashed border-[rgba(var(--accent),0.12)]",
        className
      )}
    >
      {Icon && (
        <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.06)] border border-[rgba(var(--accent),0.12)] flex items-center justify-center mb-3 text-[rgb(var(--accent))]">
          <Icon size={22} />
        </div>
      )}
      <h4 className="font-display text-[14px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/90 mb-1">
        {title}
      </h4>
      {description && (
        <p className="text-[12px] text-[rgb(var(--foreground-muted))]/65 max-w-xs leading-relaxed mb-4">
          {description}
        </p>
      )}
      {action && <div className="mt-1">{action}</div>}
    </Card>
  );
};
