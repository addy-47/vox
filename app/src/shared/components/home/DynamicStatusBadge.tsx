import { memo } from "react";
import { Shimmer } from "@/shared/components/ai-elements/Shimmer";
import { HOME_CONTROLS_COPY } from "@/data/homeCopy";
import { cn } from "@/shared/lib/utils";

export interface DynamicStatusBadgeProps {
  label: string;
  shimmer: boolean;
  className?: string;
  orbState?: string;
}

export const DynamicStatusBadge = memo<DynamicStatusBadgeProps>(({
  label,
  shimmer,
  className,
}) => {
  return (
    <div
      role="status"
      aria-live="polite"
      aria-label={HOME_CONTROLS_COPY.statusAriaLabel(label)}
      className={cn(
        "flex items-center pointer-events-none select-none transition-all duration-300",
        className
      )}
    >
      <div className="text-[14px] sm:text-[15px] font-mono font-bold tracking-[0.18em] uppercase flex items-center">
        {shimmer ? (
          <Shimmer
            duration={2.0}
            className="text-[14px] sm:text-[15px] font-mono font-bold tracking-[0.18em] uppercase text-[rgb(var(--accent))]"
          >
            {label}
          </Shimmer>
        ) : (
          <span className="text-[rgb(var(--accent))] transition-colors duration-300">
            {label}
          </span>
        )}
      </div>
    </div>
  );
});

DynamicStatusBadge.displayName = "DynamicStatusBadge";
