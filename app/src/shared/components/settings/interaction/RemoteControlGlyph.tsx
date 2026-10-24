import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface RemoteControlGlyphProps {
  label?: string;
  ariaLabel: string;
  enabled?: boolean;
  onToggle?: () => void;
}

/**
 * Clean SVG wireframe glyph for the Remote Control desk tab.
 * Wireframe mast and broadcast arcs matching the desk controls grammar.
 */
export const RemoteControlGlyph = memo(
  ({ label, ariaLabel, enabled = false, onToggle }: RemoteControlGlyphProps) => {
    const content = (
      <>
        <div
          className={cn(
            "relative w-[48px] h-[48px] transition-transform duration-300",
            onToggle && "group-hover:scale-105"
          )}
        >
          <svg
            viewBox="0 0 48 48"
            className={cn("w-full h-full overflow-visible select-none")}
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            {/* Mast */}
            <line
              x1="24"
              y1="14"
              x2="24"
              y2="38"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              className={enabled ? "opacity-90" : "opacity-35"}
            />
            {/* Base */}
            <line
              x1="16"
              y1="38"
              x2="32"
              y2="38"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              className={enabled ? "opacity-80" : "opacity-35"}
            />
            {/* Inner Signal arcs */}
            <path
              d="M 16 22 a 11 11 0 0 1 16 0"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              className={enabled ? "opacity-100 animate-pulse-slow" : "opacity-40"}
            />
            {/* Outer Signal arcs */}
            <path
              d="M 11 16 a 18 18 0 0 1 26 0"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              className={enabled ? "opacity-80" : "opacity-25"}
            />
            {/* Head */}
            <circle
              cx="24"
              cy="11"
              r="3"
              stroke="currentColor"
              strokeWidth="1.25"
              className={enabled ? "fill-[rgb(var(--accent))]/30 opacity-100" : "opacity-70"}
            />
          </svg>
        </div>

        {label && (
          <span
            className={cn(
              "mt-3 text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-colors",
              enabled
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/50"
            )}
          >
            {label}
          </span>
        )}
      </>
    );

    if (onToggle) {
      return (
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          aria-label={ariaLabel}
          onClick={onToggle}
          className={cn(
            "group flex flex-col items-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
            enabled
              ? "text-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
          )}
        >
          {content}
        </button>
      );
    }

    return (
      <div
        className={cn(
          "flex flex-col items-center shrink-0 select-none",
          enabled ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/40"
        )}
        aria-label={ariaLabel}
        role="img"
      >
        {content}
      </div>
    );
  }
);

RemoteControlGlyph.displayName = "RemoteControlGlyph";
