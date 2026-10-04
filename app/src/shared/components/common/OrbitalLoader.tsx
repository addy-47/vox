import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface ParticleOrbProps {
  size?: number;
  className?: string;
}

/**
 * Pure stroke-style particle orb SVG matching the Liquid Space design system.
 * Frameless vector geometry with delicate elliptical trajectories and nodal particles.
 */
export const ParticleOrb: React.FC<ParticleOrbProps> = ({ size = 24, className }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={cn("shrink-0", className)}
    aria-hidden="true"
  >
    {/* Central nucleus particle */}
    <circle cx="12" cy="12" r="2" fill="currentColor" opacity="0.9" />

    {/* Inner resonance ring */}
    <circle
      cx="12"
      cy="12"
      r="4.5"
      stroke="currentColor"
      strokeWidth="1"
      strokeDasharray="2 3"
      opacity="0.6"
    />

    {/* Primary orbital ellipse (-30deg) */}
    <ellipse
      cx="12"
      cy="12"
      rx="9"
      ry="3.8"
      stroke="currentColor"
      strokeWidth="1.1"
      strokeLinecap="round"
      transform="rotate(-30 12 12)"
      opacity="0.85"
    />
    {/* Nodal particle on orbit 1 */}
    <circle cx="19.8" cy="7.5" r="1.2" fill="currentColor" />

    {/* Secondary orbital ellipse (+30deg) */}
    <ellipse
      cx="12"
      cy="12"
      rx="9"
      ry="3.8"
      stroke="currentColor"
      strokeWidth="1.1"
      strokeLinecap="round"
      transform="rotate(30 12 12)"
      opacity="0.85"
    />
    {/* Nodal particle on orbit 2 */}
    <circle cx="4.2" cy="7.5" r="1.2" fill="currentColor" />

    {/* Vertical eccentric orbit (90deg) */}
    <ellipse
      cx="12"
      cy="12"
      rx="8.5"
      ry="3.2"
      stroke="currentColor"
      strokeWidth="0.9"
      strokeDasharray="3 2"
      transform="rotate(90 12 12)"
      opacity="0.5"
    />
    {/* Nodal particle on vertical orbit */}
    <circle cx="12" cy="20.5" r="1" fill="currentColor" opacity="0.75" />
  </svg>
);

export interface OrbitalLoaderProps {
  /** Size variant: "sm" (compact card/popover), "md" (standard), "lg" (full screen/page) */
  size?: "sm" | "md" | "lg";
  /** Optional custom center graphic (defaults to stroke-style ParticleOrb) */
  icon?: React.ComponentType<{ size?: number; className?: string }>;
  /** Whether to render as a full-screen fixed/absolute backdrop overlay */
  overlay?: boolean;
  /** Custom className for the container */
  className?: string;
}

/**
 * Textless, frameless ambient loading indicator per design spec §7.1.
 * Pure geometric particle core with concentric resonance rings.
 */
export const OrbitalLoader: React.FC<OrbitalLoaderProps> = memo(
  ({
    size = "md",
    icon: IconComponent = ParticleOrb,
    overlay = false,
    className,
  }) => {
    const sizeConfig = {
      sm: {
        container: "w-16 h-16",
        innerP: "p-2.5",
        iconSize: 18,
      },
      md: {
        container: "w-24 h-24",
        innerP: "p-3.5",
        iconSize: 24,
      },
      lg: {
        container: "w-28 h-28",
        innerP: "p-4",
        iconSize: 30,
      },
    }[size];

    const content = (
      <div className={cn("flex flex-col items-center justify-center select-none", className)}>
        {/* Orbital Glowing Central Core */}
        <div className={cn("relative flex items-center justify-center", sizeConfig.container)}>
          {/* Ambient outer pulse aura */}
          <div className="absolute inset-0 rounded-full bg-[rgb(var(--accent))]/10 animate-ping duration-1000" />

          {/* Clockwise rotating ring */}
          <div className="absolute inset-1.5 sm:inset-2 rounded-full border border-[rgb(var(--accent))]/25 animate-spin duration-[6000ms]" />

          {/* Counter-clockwise dashed resonance ring */}
          <div className="absolute inset-3.5 sm:inset-5 rounded-full border border-dashed border-[rgb(var(--accent))]/40 animate-spin duration-[10000ms] [animation-direction:reverse]" />

          {/* Central Glowing Orb Core */}
          <div
            className={cn(
              "relative z-10 rounded-full bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] shadow-[0_0_40px_rgba(var(--accent),0.35)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center",
              sizeConfig.innerP
            )}
          >
            <IconComponent size={sizeConfig.iconSize} className="animate-pulse text-[rgb(var(--accent))]" />
          </div>
        </div>
      </div>
    );

    if (overlay) {
      return (
        <div className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-[rgb(var(--background))]/90 backdrop-blur-3xl pointer-events-none select-none">
          {content}
        </div>
      );
    }

    return content;
  }
);

OrbitalLoader.displayName = "OrbitalLoader";
