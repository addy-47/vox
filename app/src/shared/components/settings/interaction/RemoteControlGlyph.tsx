import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface RemoteControlGlyphProps {
  label: string;
  ariaLabel: string;
}

/**
 * Placeholder glyph for the Remote Control desk tab.
 * Same wireframe grammar as the other desk controls (globe, ghost, clock):
 * monochrome 48x48 line art with a status badge underneath.
 */
export const RemoteControlGlyph = memo(({ label, ariaLabel }: RemoteControlGlyphProps) => (
  <div
    className="flex flex-col items-center shrink-0 select-none text-[rgb(var(--foreground-muted))]/40"
    aria-label={ariaLabel}
    role="img"
  >
    <div className="relative w-[48px] h-[48px]">
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
          className="opacity-35"
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
          className="opacity-35"
        />
        {/* Signal arcs */}
        <path
          d="M 16 22 a 11 11 0 0 1 16 0"
          stroke="currentColor"
          strokeWidth="1.25"
          strokeLinecap="round"
          className="opacity-70"
        />
        <path
          d="M 11 16 a 18 18 0 0 1 26 0"
          stroke="currentColor"
          strokeWidth="1.25"
          strokeLinecap="round"
          className="opacity-35"
        />
        {/* Head */}
        <circle cx="24" cy="11" r="3" stroke="currentColor" strokeWidth="1.25" className="opacity-70" />
      </svg>
    </div>

    <span className="mt-3 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none text-[rgb(var(--foreground-muted))]/50">
      {label}
    </span>
  </div>
));

RemoteControlGlyph.displayName = "RemoteControlGlyph";
