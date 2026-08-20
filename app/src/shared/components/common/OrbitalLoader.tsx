import React, { memo } from "react";
import { Sparkles, LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface OrbitalLoaderProps {
  /** Main title / action label e.g. "Building memory graph...", "Synchronizing...", "Loading history..." */
  title?: string;
  /** Primary subtitle e.g. "12,450 nodes · 48,200 edges" */
  subtitle?: string;
  /** Secondary micro-text e.g. "Optimizing layout and relationships" */
  statusText?: string;
  /** Size variant: "sm" (compact card/popover), "md" (standard), "lg" (full screen/page) */
  size?: "sm" | "md" | "lg";
  /** Optional custom center icon (defaults to Sparkles) */
  icon?: LucideIcon;
  /** Whether to render as a full-screen fixed/absolute backdrop overlay */
  overlay?: boolean;
  /** Custom className for the container */
  className?: string;
}

export const OrbitalLoader: React.FC<OrbitalLoaderProps> = memo(
  ({
    title = "Loading...",
    subtitle,
    statusText,
    size = "md",
    icon: IconComponent = Sparkles,
    overlay = false,
    className,
  }) => {
    // Sizing scale maps
    const sizeConfig = {
      sm: {
        container: "w-16 h-16 mb-4",
        innerP: "p-2.5",
        iconSize: 18,
        titleClass: "text-[13px]",
        subtitleClass: "text-[11px]",
        statusClass: "text-[11px] mt-1",
      },
      md: {
        container: "w-24 h-24 mb-6",
        innerP: "p-3.5",
        iconSize: 24,
        titleClass: "text-[14px]",
        subtitleClass: "text-[12px]",
        statusClass: "text-[11px] mt-1.5",
      },
      lg: {
        container: "w-28 h-28 mb-8",
        innerP: "p-4",
        iconSize: 30,
        titleClass: "text-[15px]",
        subtitleClass: "text-[12px]",
        statusClass: "text-[11px] mt-2",
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
              "relative z-10 rounded-full bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] shadow-[0_0_40px_rgba(var(--accent),0.35)] border border-[rgba(var(--accent),0.3)]",
              sizeConfig.innerP
            )}
          >
            <IconComponent size={sizeConfig.iconSize} className="animate-pulse text-[rgb(var(--accent))]" />
          </div>
        </div>

        {/* Clean Borderless Modern Typography */}
        {(title || subtitle || statusText) && (
          <div className="flex flex-col items-center text-center gap-1">
            {title && (
              <h3
                className={cn(
                  "font-display font-black tracking-wide text-[rgb(var(--foreground))] drop-shadow-sm",
                  sizeConfig.titleClass
                )}
              >
                {title}
              </h3>
            )}
            {subtitle && (
              <p
                className={cn(
                  "font-sans font-medium text-[rgb(var(--foreground-muted))]",
                  sizeConfig.subtitleClass
                )}
              >
                {subtitle}
              </p>
            )}
            {statusText && (
              <p
                className={cn(
                  "font-mono font-semibold text-[rgb(var(--accent))] tracking-wider uppercase opacity-80",
                  sizeConfig.statusClass
                )}
              >
                {statusText}
              </p>
            )}
          </div>
        )}
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
