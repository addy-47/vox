import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface PrivateModeGlyphProps {
  enabled: boolean;
  onToggle: () => void;
  label: string;
  ariaLabel: string;
}

/**
 * Line-style ghost glyph for the private session (incognito) toggle.
 * Matches the wireframe grammar of the other desk controls (globe, clock,
 * gate): monochrome stroke, accent when engaged, status badge underneath.
 */
export const PrivateModeGlyph = memo(
  ({ enabled, onToggle, label, ariaLabel }: PrivateModeGlyphProps) => (
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
      <div
        className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105"
        style={{
          animation: enabled ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
        }}
      >
        <svg
          viewBox="0 0 48 48"
          className="w-full h-full overflow-visible select-none"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Ghost body */}
          <path
            d="M 13 23 a 11 11 0 0 1 22 0 v 15 l -5.5 -4 l -5.5 4 l -5.5 -4 l -5.5 4 z"
            stroke="currentColor"
            strokeWidth="1.25"
            strokeLinejoin="round"
            className={cn(
              "transition-opacity duration-300",
              enabled ? "opacity-90" : "opacity-35"
            )}
          />
          {/* Eyes */}
          <circle
            cx="19"
            cy="21"
            r="1.6"
            fill="currentColor"
            className={cn(
              "transition-opacity duration-300",
              enabled ? "opacity-85" : "opacity-30"
            )}
          />
          <circle
            cx="29"
            cy="21"
            r="1.6"
            fill="currentColor"
            className={cn(
              "transition-opacity duration-300",
              enabled ? "opacity-85" : "opacity-30"
            )}
          />
        </svg>
      </div>

      <span
        className={cn(
          "mt-3 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300",
          enabled
            ? "text-[rgb(var(--accent))]"
            : "text-[rgb(var(--foreground-muted))]/50"
        )}
      >
        {label}
      </span>
    </button>
  )
);

PrivateModeGlyph.displayName = "PrivateModeGlyph";
