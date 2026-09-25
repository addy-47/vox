import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface TransliterationToggleProps {
  enabled: boolean;
  onToggle: () => void;
  label?: string;
  ariaLabel?: string;
}

/**
 * Interactive SVG toggle for real-time script transliteration.
 * Follows the same philosophy as WebSearchGlobe:
 * - Disabled: Devanagari character (अ) remains standalone in its native script.
 * - Enabled: Devanagari (अ) maps dynamically to Latin (a) via a directional vector (अ → a).
 */
export const TransliterationToggle = memo(
  ({
    enabled,
    onToggle,
    label,
    ariaLabel = "Toggle script transliteration",
  }: TransliterationToggleProps) => (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      aria-label={ariaLabel}
      onClick={onToggle}
      className={cn(
        "group flex flex-col items-center justify-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
        enabled
          ? "text-[rgb(var(--accent))]"
          : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
      )}
    >
      {/* SVG Canvas for Script Mapping */}
      <div
        className="relative w-[76px] h-[46px] transition-transform duration-300 group-hover:scale-105"
        style={{
          animation: enabled ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
        }}
      >
        <svg
          viewBox="0 0 76 46"
          className="w-full h-full overflow-visible select-none"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {enabled ? (
            /* ── Enabled State: Devanagari (अ) → Latin (a) Transfer ── */
            <g className="transition-all duration-300 animate-fade-in">
              {/* Outer Wireframe Capsule Boundary */}
              <rect
                x="3"
                y="3"
                width="70"
                height="40"
                rx="10"
                stroke="currentColor"
                strokeWidth="1.1"
                className="opacity-70"
              />

              {/* Devanagari Source Glyph (अ) */}
              <text
                x="20"
                y="24"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="17"
                fontWeight="700"
                fill="currentColor"
                className="select-none"
                style={{ fontFamily: "'Noto Sans Devanagari', 'Mukta', sans-serif" }}
              >
                अ
              </text>

              {/* Directional Flow Vector (→) */}
              <g className="opacity-80">
                <line
                  x1="32"
                  y1="23"
                  x2="44"
                  y2="23"
                  stroke="currentColor"
                  strokeWidth="1.2"
                  strokeDasharray="2 1.5"
                />
                <path
                  d="M 41 20 L 45 23 L 41 26"
                  stroke="currentColor"
                  strokeWidth="1.25"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </g>

              {/* Latin Target Glyph (a) */}
              <text
                x="56"
                y="23"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="17"
                fontWeight="800"
                fill="currentColor"
                className="select-none"
                style={{ fontFamily: "monospace, system-ui, sans-serif" }}
              >
                a
              </text>

              {/* Accent micro-markers */}
              <circle cx="20" cy="38" r="1.2" fill="currentColor" opacity="0.6" />
              <circle cx="56" cy="38" r="1.2" fill="currentColor" opacity="0.6" />
            </g>
          ) : (
            /* ── Disabled State: Devanagari remains untransliterated (अ) ── */
            <g className="transition-all duration-300">
              {/* Stationary Outer Perimeter */}
              <rect
                x="14"
                y="3"
                width="48"
                height="40"
                rx="10"
                stroke="currentColor"
                strokeWidth="1.1"
                strokeDasharray="3 2"
                className="opacity-30"
              />

              {/* Centered Solo Devanagari Glyph */}
              <text
                x="38"
                y="23"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="19"
                fontWeight="700"
                fill="currentColor"
                className="opacity-55 select-none"
                style={{ fontFamily: "'Noto Sans Devanagari', 'Mukta', sans-serif" }}
              >
                अ
              </text>
            </g>
          )}
        </svg>
      </div>

      {/* Label Badge with distinct top spacing */}
      <span
        className={cn(
          "mt-2 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300",
          enabled
            ? "text-[rgb(var(--accent))]"
            : "text-[rgb(var(--foreground-muted))]/50"
        )}
      >
        {label || (enabled ? "ENABLED" : "DISABLED")}
      </span>
    </button>
  )
);

TransliterationToggle.displayName = "TransliterationToggle";
