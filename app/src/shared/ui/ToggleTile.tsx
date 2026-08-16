import React from "react";
import { cn } from "@/shared/lib/utils";

export interface ToggleTileProps {
  title: string;
  active: boolean;
  activeLabel: string;
  inactiveLabel: string;
  activeSublabel?: string;
  inactiveSublabel?: string;
  icon?: React.ElementType;
  onToggle: () => void;
  layoutMode?: "full-max" | "full-min" | "small";
  visualizer?: React.ReactNode;
  className?: string;
}

export const ToggleTile: React.FC<ToggleTileProps> = ({
  title,
  active,
  activeLabel,
  inactiveLabel,
  activeSublabel,
  inactiveSublabel,
  icon: Icon,
  onToggle,
  layoutMode = "full-max",
  visualizer,
  className,
}) => {
  const isSmall = layoutMode === "small";

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      onToggle();
    }
  };

  return (
    <div
      role="switch"
      aria-checked={active}
      aria-label={title}
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={handleKeyDown}
      className={cn(
        "group flex items-center w-full h-[85px] relative focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] rounded-xl cursor-pointer select-none",
        className
      )}
    >
      <div
        className="flex-1 p-3 rounded-xl group-hover:rounded-r-none border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] group-hover:border-[rgba(var(--accent),0.2)] group-hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between h-full min-w-0"
      >
        <div className="flex items-center justify-between gap-2">
          <span className="text-[12px] font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70 whitespace-nowrap uppercase">
            {title}
          </span>
          {Icon && (
            <Icon
              size={16}
              className={
                active
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/40"
              }
            />
          )}
        </div>

        <div className="flex items-end justify-between mt-2">
          <div className="flex flex-col min-w-0">
            <span className="text-[12px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none truncate">
              {active ? activeLabel : inactiveLabel}
            </span>
            {(activeSublabel || inactiveSublabel) && (
              <span className="text-[12px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none truncate">
                {active ? activeSublabel : inactiveSublabel}
              </span>
            )}
          </div>

          {visualizer ? (
            <div className="h-4 flex items-end shrink-0">{visualizer}</div>
          ) : (
            <div className="h-4 flex items-end shrink-0">
              <span
                className={cn(
                  "w-2 h-2 rounded-full transition-all",
                  active
                    ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.6)] animate-pulse"
                    : "bg-[rgb(var(--foreground-muted))]/30"
                )}
              />
            </div>
          )}
        </div>
      </div>

      {/* Slide-out toggle side panel */}
      <div
        className="h-full w-0 group-hover:w-[38px] opacity-0 group-hover:opacity-100 flex items-center justify-center bg-[rgba(var(--accent),0.05)] border border-transparent group-hover:border-[rgba(var(--accent),0.15)] group-hover:border-l-transparent rounded-r-xl transition-all duration-300 overflow-hidden shrink-0"
      >
        <span className="text-[11px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--accent))] rotate-90 whitespace-nowrap">
          {isSmall ? "TAP" : "TOGGLE"}
        </span>
      </div>
    </div>
  );
};
