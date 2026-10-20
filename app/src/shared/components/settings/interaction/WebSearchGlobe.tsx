import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface WebSearchGlobeProps {
  enabled: boolean;
  onToggle: () => void;
  label: string;
  ariaLabel: string;
}

/**
 * Clean SVG wireframe globe toggle for Web Search.
 * Simple, crisp vector geometry with zero radar artifacts, zero excessive glow,
 * and clear padding between the globe and label.
 */
export const WebSearchGlobe = memo(
  ({ enabled, onToggle, label, ariaLabel }: WebSearchGlobeProps) => (
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
      {/* Crisp 50x50 SVG Vector Globe */}
      <div
        className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105"
        style={{
          animation: enabled ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
        }}
      >
        <svg
          viewBox="0 0 52 52"
          className="w-full h-full overflow-visible select-none"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Outer perimeter ring */}
          <circle
            cx="26"
            cy="26"
            r="22"
            stroke="currentColor"
            strokeWidth="1.25"
            className={cn("transition-opacity duration-300", enabled ? "opacity-90" : "opacity-35")}
          />

          {/* Equator & Latitudes */}
          <line
            x1="4"
            y1="26"
            x2="48"
            y2="26"
            stroke="currentColor"
            strokeWidth="0.9"
            className={cn("transition-opacity duration-300", enabled ? "opacity-60" : "opacity-25")}
          />
          <ellipse
            cx="26"
            cy="16"
            rx="18.5"
            ry="6"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn("transition-opacity duration-300", enabled ? "opacity-45" : "opacity-20")}
          />
          <ellipse
            cx="26"
            cy="36"
            rx="18.5"
            ry="6"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn("transition-opacity duration-300", enabled ? "opacity-45" : "opacity-20")}
          />

          {/* Prime Axis & Longitudes */}
          <line
            x1="26"
            y1="4"
            x2="26"
            y2="48"
            stroke="currentColor"
            strokeWidth="0.9"
            className={cn("transition-opacity duration-300", enabled ? "opacity-60" : "opacity-25")}
          />
          <ellipse
            cx="26"
            cy="26"
            rx="9"
            ry="22"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn("transition-opacity duration-300", enabled ? "opacity-45" : "opacity-20")}
          />
          <ellipse
            cx="26"
            cy="26"
            rx="16"
            ry="22"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn("transition-opacity duration-300", enabled ? "opacity-45" : "opacity-20")}
          />

        </svg>
      </div>

      {/* Label Badge with distinct top padding from globe */}
      <span
        className={cn(
          "mt-3 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300",
          enabled
            ? " text-[rgb(var(--accent))]"
            : " text-[rgb(var(--foreground-muted))]/50"
        )}
      >
        {label}
      </span>
    </button>
  )
);

WebSearchGlobe.displayName = "WebSearchGlobe";
