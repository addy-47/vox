import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface VoxLogoLoaderProps {
  /** Size variant or explicit pixel size */
  size?: "sm" | "md" | "lg" | number;
  className?: string;
  animated?: boolean;
}

const SIZE_MAP: Record<"sm" | "md" | "lg", number> = {
  sm: 28,
  md: 44,
  lg: 64,
};

/**
 * Minimalist stroke-style Vox logo SVG loader per design-spec §7.1.
 * Ultra-lean vector geometry with zero external dependencies, zero typography,
 * and pure CSS hardware-accelerated stroke flow + breathing glow.
 */
export const VoxLogoLoader: React.FC<VoxLogoLoaderProps> = memo(
  ({ size = "md", className, animated = true }) => {
    const pixelSize = typeof size === "number" ? size : SIZE_MAP[size] ?? 44;

    return (
      <div
        className={cn(
          "inline-flex items-center justify-center select-none shrink-0",
          animated && "vox-logo-breathing",
          className
        )}
        style={{ width: pixelSize, height: pixelSize }}
        role="status"
        aria-label="Loading"
      >
        <svg
          width={pixelSize}
          height={pixelSize}
          viewBox="0 0 32 32"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          className="w-full h-full text-[rgb(var(--accent))]"
          aria-hidden="true"
        >
          {/* Symmetrical vertical sound equalizer lines */}
          <g className="opacity-35" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
            <line x1="8" y1="12" x2="8" y2="20" />
            <line x1="12" y1="8" x2="12" y2="24" />
            <line x1="16" y1="5" x2="16" y2="27" />
            <line x1="20" y1="8" x2="20" y2="24" />
            <line x1="24" y1="12" x2="24" y2="20" />
          </g>

          {/* Signature Vox central waveform with iconic 'V' dip */}
          <path
            d="M 6.5 16 C 8 16 8.5 11 10 11 C 11.5 11 12 18.5 13.5 18.5 C 14.5 18.5 15.2 24 16 24 C 16.8 24 17.5 18.5 18.5 18.5 C 20 18.5 20.5 11 22 11 C 23.5 11 24 16 25.5 16"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={cn(animated && "vox-logo-wave-path")}
          />

          {/* Terminal axis nodes */}
          <circle cx="4.5" cy="16" r="1.15" fill="currentColor" opacity="0.85" />
          <circle cx="27.5" cy="16" r="1.15" fill="currentColor" opacity="0.85" />
        </svg>
      </div>
    );
  }
);

VoxLogoLoader.displayName = "VoxLogoLoader";
